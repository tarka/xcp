use libc;
use log::{debug, info};
use std::cmp;
use std::fs::{create_dir, create_dir_all, File};
use std::io;
use std::io::ErrorKind as IOKind;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::thread;
use walkdir::WalkDir;

use indicatif::{ProgressBar, ProgressStyle};


use crate::errors::{io_err, Result, XcpError};
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

trait StoredValue<V, T> {
    fn set(&self, bytes: V) -> T;
    fn value(&self) -> V;
}


#[derive(Debug)]
enum Operation {
    Copy(PathBuf, PathBuf),
    CreateDir(PathBuf),
    End,
}

#[derive(Debug, Clone)]
enum StatusUpdate {
    Copied(u64),
    Size(u64),
}

impl StoredValue<u64, StatusUpdate> for StatusUpdate {
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


#[derive(Debug)]
struct Batcher {
    sender: Option<mpsc::Sender<StatusUpdate>>,
    stat: StatusUpdate,
    batch_size: u64,
}


impl Batcher {
    fn send(&mut self, bytes: u64) -> Result<()> {
        if let Some(sender) = &self.sender {
            let curr = self.stat.value() + bytes;
            self.stat = self.stat.set(curr);

            if curr >= self.batch_size {
                sender.send(self.stat.clone())?;
                self.stat = self.stat.set(0);
            }
        }
        Ok(())
    }
}


/* **** File operations **** */

fn copy_file(from: &Path, to: &Path, updates: &mut Batcher) -> Result<u64> {
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
        updates.send(result)?;
    }
    outfd.set_permissions(perm)?;
    Ok(written)
}


fn copy_worker(work: mpsc::Receiver<Operation>, mut updates: Batcher) -> Result<()> {
    debug!("Starting copy worker {:?}", thread::current().id());
    for op in work {
        debug!("Received operation {:?}", op);

        match op {
            Operation::Copy(from, to) => {
                // FIXME: If we implement parallel copies (which may
                // improve performance on some SSD configurations) we
                // should also created the parent directory, and the
                // dir-create operation could be out of order.

                info!("Worker: Copy {:?} -> {:?}", from, to);
                let res = copy_file(&from, &to, &mut updates);
                //updates.send(res?)?;
            }

            Operation::CreateDir(dir) => {
                info!("Worker: Creating directory: {:?}", dir);
                create_dir_all(&dir)?;
                updates.send(dir.metadata()?.len())?;
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

fn tree_walker(
    source: PathBuf,
    dest: PathBuf,
    work_tx: mpsc::Sender<Operation>,
    mut updates: Batcher,
) -> Result<()> {
    debug!("Starting walk worker {:?}", thread::current().id());

    let sourcedir = source.components().last().ok_or(XcpError::InvalidSource {
        msg: "Failed to find source directory name.",
    })?;
    let basedir = dest.join(sourcedir);

    for entry in WalkDir::new(&source).into_iter() {
        debug!("Got tree entry {:?}", entry);
        // FIXME: Return errors to the master thread.
        let from = entry?;
        let meta = from.metadata()?;
        let path = from.path().strip_prefix(&source)?;
        let target = basedir.join(&path);

        match meta.file_type().to_enum() {
            FileType::File => {
                debug!("Send copy operation {:?} to {:?}", from.path(), target);
                updates.send(meta.len())?;
                work_tx.send(Operation::Copy(from.path().to_path_buf(), target))?;
            }

            FileType::Dir => {
                debug!(
                    "Send create-dir operation {:?} to {:?}",
                    from.path(),
                    target
                );
                work_tx.send(Operation::CreateDir(target))?;
                updates.send(from.path().metadata()?.len())?;
            }

            FileType::Symlink => {}
        };
    }

    work_tx.send(Operation::End)?;
    debug!("Walk-worker finished: {:?}", thread::current().id());
    Ok(())
}

pub fn copy_tree(opts: &Opts) -> Result<()> {
    let (work_tx, work_rx) = mpsc::channel();
    let (stat_tx, stat_rx) = mpsc::channel();
    let copy_stat = Batcher {
        sender: Some(stat_tx.clone()),
        stat: StatusUpdate::Copied(0),
        batch_size: 1000 * 4096,
    };
    let size_stat = Batcher {
        sender: Some(stat_tx),
        stat: StatusUpdate::Size(0),
        batch_size: 1000 * 4096,
    };
    let source = opts.source.clone();
    let dest = opts.dest.clone();

    let _copy_worker = thread::spawn(move || copy_worker(work_rx, copy_stat));
    let _walk_worker = thread::spawn(move || tree_walker(source, dest, work_tx, size_stat));

    let mut copied = 0;
    let mut total = 0;

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:80.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"),
    );

    for stat in stat_rx {
        match stat {
            StatusUpdate::Size(s) => {
                total += s;
                pb.set_length(total);
            }
            StatusUpdate::Copied(s) => {
                copied += s;
                pb.set_position(copied);
            }
        }
    }

    pb.finish_with_message("Copy-tree complete");
    debug!("Copy-tree complete");
    Ok(())
}


// FIXME: This could be changed to use copy_tree, but involves some
// special cases, e.g. when target file is a different name from the
// source.
pub fn copy_single_file(opts: &Opts) -> Result<()> {
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

    let mut copy_stat = Batcher {
        sender: None,
        stat: StatusUpdate::Copied(0),
        batch_size: usize::max_value() as u64,
    };
    copy_file(&opts.source, &dest, &mut copy_stat)?;

    Ok(())
}