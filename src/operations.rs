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

use libc;

use crate::Opts;
use crate::errors::{io_err, Result, XcpError};
use crate::utils::{FileType, ToFileType};

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

fn r_copy_file_range(infd: &File, outfd: &File, bytes: usize) -> Result<u64> {
    let r = unsafe {
        copy_file_range(
            infd.as_raw_fd(),
            null_mut(),
            outfd.as_raw_fd(),
            null_mut(),
            bytes,
            0,
        )
    };
    match r {
        -1 => Err(io::Error::last_os_error().into()),
        _ => Ok(r as u64),
    }
}

fn copy_file(from: &Path, to: &Path) -> Result<u64> {
    let infd = File::open(from)?;
    let outfd = File::create(to)?;
    let (perm, len) = {
        let metadata = infd.metadata()?;
        (metadata.permissions(), metadata.len())
    };

    let mut written = 0u64;
    while written < len {
        let bytes_to_copy = cmp::min(len - written, usize::max_value() as u64) as usize;
        let result = r_copy_file_range(&infd, &outfd, bytes_to_copy)?;
        written += result;
    }
    outfd.set_permissions(perm)?;
    Ok(written)
}


/* **** Structured operations **** */

#[derive(Debug)]
enum Operation {
    Copy(PathBuf, PathBuf),
    CreateDir(PathBuf),
    End,
}

#[derive(Debug)]
enum OpStatus {
    Copied(Result<(u64)>),
    Size(Result<u64>)
}


fn copy_worker(work: mpsc::Receiver<Operation>,
               results: mpsc::Sender<OpStatus>)
               -> Result<()>
{
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
                let res = copy_file(&from, &to);
                results.send(OpStatus::Copied(res))?;
            }

            Operation::CreateDir(dir) => {
                info!("Worker: Creating directory: {:?}", dir);
                create_dir_all(dir)?;
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

fn tree_walker(source: PathBuf, dest: PathBuf,
               work_tx: mpsc::Sender<Operation>,
               stat_tx: mpsc::Sender<OpStatus>)
               -> Result<()>
{
    debug!("Starting walk worker {:?}", thread::current().id());

    let sourcedir = source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource {
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
                work_tx.send(Operation::Copy(from.path().to_path_buf(), target))?;
                stat_tx.send(OpStatus::Size(Ok(meta.len())))?;
            },

            FileType::Dir => {
                debug!("Send create-dir operation {:?} to {:?}", from.path(), target);
                work_tx.send(Operation::CreateDir(target))?;
            },

            FileType::Symlink => {
            },
        };
    }

    debug!("Walk-worker finished: {:?}", thread::current().id());
    Ok(())
}

pub fn copy_tree(opts: &Opts) -> Result<()> {

    let (work_tx, work_rx) = mpsc::channel();
    let (stat_tx, stat_rx) = mpsc::channel();
    let stat_tx2 = stat_tx.clone();
    let source = opts.source.clone();
    let dest = opts.dest.clone();

    let _copy_worker = thread::spawn(move || copy_worker(work_rx, stat_tx));
    let _walk_worker = thread::spawn(move || tree_walker(source, dest,
                                                         work_tx, stat_tx2));

    for n in stat_rx {
        debug!("Received {:?} from the main thread!", n);
    }

    debug!("Copy-tree complete");
    Ok(())
}


// FIXME: Could just use copy_tree if works on single files?
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

    copy_file(&opts.source, &dest)?;

    Ok(())
}
