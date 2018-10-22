use ignore::gitignore::{Gitignore, GitignoreBuilder};
use libc;
use log::{debug, error, info};
use std::cmp;
use std::fs::{create_dir_all, read_link, File};
use std::io;
use std::io::ErrorKind as IOKind;
use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::thread;
use walkdir::{DirEntry, WalkDir};

use crate::errors::{io_err, Result, XcpError};
use crate::progress::{ProgressBar, iprogress_bar};
use crate::utils::{FileType, ToFileType};
use crate::Opts;


/* **** Low level operations **** */

// Assumes Linux kernel >= 4.5.
#[cfg(feature = "kernel_copy_file_range")]
unsafe fn copy_file_range(
    fd_in: libc::c_int,
    off_in: *mut libc::loff_t,
    fd_out: libc::c_int,
    off_out: *mut libc::loff_t,
    len: libc::size_t,
    flags: libc::c_uint,
) -> libc::ssize_t {
    libc::syscall(
        libc::SYS_copy_file_range,
        fd_in,
        off_in,
        fd_out,
        off_out,
        len,
        flags,
    ) as libc::ssize_t
}

// Requires GlibC >= 2.27
#[cfg(not(feature = "kernel_copy_file_range"))]
extern "C" {
    fn copy_file_range(
        fd_in: libc::c_int,
        off_in: libc::loff_t,
        fd_out: libc::c_int,
        off_out: libc::loff_t,
        len: libc::size_t,
        flags: libc::c_uint,
    ) -> libc::ssize_t;
}

fn r_copy_file_range(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
    let r = unsafe {
        copy_file_range(
            infd.as_raw_fd(),
            null_mut(),
            outfd.as_raw_fd(),
            null_mut(),
            bytes as usize,
            0,
        )
    };
    match r {
        -1 => Err(io::Error::last_os_error().into()),
        _ => Ok(r as u64),
    }
}


/* **** Progress operations **** */


#[derive(Debug)]
enum Operation {
    Copy(PathBuf, PathBuf),
    Link(PathBuf, PathBuf),
    CreateDir(PathBuf),
    End,
}

#[derive(Debug, Clone)]
enum StatusUpdate {
    Copied(u64),
    Size(u64),
}

impl StatusUpdate {
    fn set(&self, bytes: u64) -> StatusUpdate {
        match self {
            StatusUpdate::Copied(_) => StatusUpdate::Copied(bytes),
            StatusUpdate::Size(_) => StatusUpdate::Size(bytes),
        }
    }
    fn value(&self) -> u64 {
        match self {
            StatusUpdate::Copied(v) => *v,
            StatusUpdate::Size(v) => *v,
        }
    }
}


trait Updater<T> {
    fn update(&mut self, update: T) -> Result<()>;
}


struct BatchUpdater {
    sender: Box<Updater<Result<StatusUpdate>> + Send>,
    stat: StatusUpdate,
    batch_size: u64,
}


impl Updater<Result<u64>> for BatchUpdater {
    fn update(&mut self, status: Result<u64>) -> Result<()> {
        match status {
            Ok(bytes) => {
                let curr = self.stat.value() + bytes;
                self.stat = self.stat.set(curr);

                if curr >= self.batch_size {
                    self.sender.update(Ok(self.stat.clone()))?;
                    self.stat = self.stat.set(0);
                }
            }
            Err(e) => {
                self.sender.update(Err(e))?;
            }
        }
        Ok(())
    }
}


impl Updater<Result<StatusUpdate>> for mpsc::Sender<Result<StatusUpdate>> {
    fn update(&mut self, update: Result<StatusUpdate>) -> Result<()> {
        Ok(self.send(update)?)
    }
}


#[allow(dead_code)]
struct NopUpdater {}

impl Updater<Result<StatusUpdate>> for NopUpdater {
    fn update(&mut self, _update: Result<StatusUpdate>) -> Result<()> {
        Ok(())
    }
}



struct ProgressUpdater {
    pb: ProgressBar,
    written: u64,
}

impl Updater<Result<StatusUpdate>> for ProgressUpdater {
    fn update(&mut self, update: Result<StatusUpdate>) -> Result<()> {
        if let Ok(StatusUpdate::Copied(bytes)) = update {
            self.written += bytes;
            self.pb.set_position(self.written);
        }
        Ok(())
    }
}


/* **** File operations **** */

fn copy_file(from: &Path, to: &Path, updates: &mut BatchUpdater) -> Result<u64> {
    let infd = File::open(from)?;
    let outfd = File::create(to)?;
    let (perm, len) = {
        let metadata = infd.metadata()?;
        (metadata.permissions(), metadata.len())
    };

    let mut written = 0u64;
    while written < len {
        let bytes_to_copy = cmp::min(len - written, updates.batch_size);
        let result = r_copy_file_range(&infd, &outfd, bytes_to_copy)?;
        written += result;
        updates.update(Ok(result))?;
    }
    outfd.set_permissions(perm)?;
    Ok(written)
}


fn copy_worker(work: mpsc::Receiver<Operation>, mut updates: BatchUpdater) -> Result<()> {
    debug!("Starting copy worker {:?}", thread::current().id());
    for op in work {
        debug!("Received operation {:?}", op);

        // FIXME: If we implement parallel copies (which may
        // improve performance on some SSD configurations) we
        // should also created the parent directory, and the
        // dir-create operation could be out of order.
        match op {
            Operation::Copy(from, to) => {
                info!("Worker: Copy {:?} -> {:?}", from, to);
                let _res = copy_file(&from, &to, &mut updates);
            }

            Operation::Link(from, to) => {
                info!("Worker: Symlink {:?} -> {:?}", from, to);
                let _res = symlink(&from, &to);
            }

            Operation::CreateDir(dir) => {
                info!("Worker: Creating directory: {:?}", dir);
                create_dir_all(&dir)?;
                updates.update(Ok(dir.metadata()?.len()))?;
            }

            Operation::End => {
                info!("Worker received shutdown command.");
                break;
            }
        }
    }
    debug!("Copy worker {:?} shutting down", thread::current().id());
    Ok(())
}


fn ignore_filter(entry: &DirEntry, ignore: &Option<Gitignore>) -> bool {
    match ignore {
        None => true,
        Some(gi) => {
            let path = entry.path();
            let m = gi.matched(&path, path.is_dir());
            !m.is_ignore()
        }
    }
}


fn tree_walker(
    opts: Opts,
    work_tx: mpsc::Sender<Operation>,
    mut updates: BatchUpdater,
) -> Result<()> {
    debug!("Starting walk worker {:?}", thread::current().id());

    let sourcedir = opts
        .source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource {
            msg: "Failed to find source directory name.",
        })?;

    let target_base = if opts.dest.exists() {
        opts.dest.join(sourcedir)
    } else {
        opts.dest.clone()
    };


    let gitignore = if opts.gitignore {
        let mut builder = GitignoreBuilder::new(&opts.source);
        builder.add(opts.source.join(".gitignore"));
        let ignore = builder.build()?;
        Some(ignore)
    } else {
        None
    };

    let witer = WalkDir::new(&opts.source)
        .into_iter()
        .filter_entry(|e| ignore_filter(e, &gitignore));

    for entry in witer {
        debug!("Got tree entry {:?}", entry);
        let e = entry?;
        let from = e.into_path();
        let meta = from.symlink_metadata()?;
        let path = from.strip_prefix(&opts.source)?;
        let target = target_base.join(&path);

        if target.exists() && opts.noclobber {
            work_tx.send(Operation::End)?;
            updates.update(Err(XcpError::DestinationExists { path: target }.into()))?;
            return Err(XcpError::EarlyShutdown {
                msg: "Path exists and --no-clobber set.",
            }
            .into());
        }

        match meta.file_type().to_enum() {
            FileType::File => {
                debug!("Send copy operation {:?} to {:?}", from, target);
                updates.update(Ok(meta.len()))?;
                work_tx.send(Operation::Copy(from, target))?;
            }

            FileType::Symlink => {
                let lfile = read_link(from)?;
                debug!("Send symlink operation {:?} to {:?}", lfile, target);
                work_tx.send(Operation::Link(lfile, target))?;
            }

            FileType::Dir => {
                debug!("Send create-dir operation {:?} to {:?}", from, target);
                work_tx.send(Operation::CreateDir(target))?;
                updates.update(Ok(from.metadata()?.len()))?;
            }

            FileType::Unknown => {
                error!("Unknown filetype found; this should never happen!");
                work_tx.send(Operation::End)?;
                updates.update(Err(XcpError::UnknownFiletype { path: target }.into()))?;
            }
        };
    }

    work_tx.send(Operation::End)?;
    debug!("Walk-worker finished: {:?}", thread::current().id());
    Ok(())
}

pub fn copy_tree(opts: Opts) -> Result<()> {
    let (work_tx, work_rx) = mpsc::channel();
    let (stat_tx, stat_rx) = mpsc::channel();

    let (pb, batch_size) = if opts.noprogress {
        (ProgressBar::Nop, usize::max_value() as u64)
    } else {
        (iprogress_bar(0), 1024 * 4096)
    };

    let copy_stat = BatchUpdater {
        sender: Box::new(stat_tx.clone()),
        stat: StatusUpdate::Copied(0),
        batch_size: batch_size,
    };
    let size_stat = BatchUpdater {
        sender: Box::new(stat_tx),
        stat: StatusUpdate::Size(0),
        batch_size: batch_size,
    };

    let _copy_worker = thread::spawn(move || copy_worker(work_rx, copy_stat));
    let _walk_worker = thread::spawn(move || tree_walker(opts, work_tx, size_stat));

    let mut copied = 0;
    let mut total = 0;

    for stat in stat_rx {
        match stat? {
            StatusUpdate::Size(s) => {
                total += s;
                pb.set_size(total);
            }
            StatusUpdate::Copied(s) => {
                copied += s;
                pb.set_position(copied);
            }
        }
    }
    // FIXME: We should probably join the threads and consume any errors.

    pb.end();
    debug!("Copy-tree complete");

    Ok(())
}


// FIXME: This could be changed to use copy_tree, but involves some
// special cases, e.g. when target file is a different name from the
// source.
pub fn copy_single_file(opts: Opts) -> Result<()> {
    let dest = if opts.dest.is_dir() {
        let fname = opts.source.file_name().ok_or(XcpError::UnknownFilename)?;
        opts.dest.join(fname)
    } else {
        opts.dest.clone()
    };

    if dest.is_file() && opts.noclobber {
        return Err(io_err(
            IOKind::AlreadyExists,
            "Destination file exists and no-clobber is set.",
        ));
    }


    let mut copy_stat = if opts.noprogress {
        BatchUpdater {
            sender: Box::new(NopUpdater {}),
            stat: StatusUpdate::Copied(0),
            batch_size: usize::max_value() as u64,
        }

    } else {
        let size = opts.source.metadata()?.len();
        BatchUpdater {
            sender: Box::new(ProgressUpdater {
                pb: iprogress_bar(size),
                written: 0,
            }),
            stat: StatusUpdate::Copied(0),
            batch_size: size / 10,
        }
    };

    copy_file(&opts.source, &dest, &mut copy_stat)?;

    Ok(())
}
