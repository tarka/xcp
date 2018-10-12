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
    End,
}

#[derive(Debug)]
enum OpResult {
    Copied(Result<(u64)>)
}


fn copy_worker(work: mpsc::Receiver<Operation>,
               results: mpsc::Sender<OpResult>) -> Result<()>
{
    debug!("Starting worker {:?}", thread::current().id());
    for op in work {
        debug!("Received operation {:?}", op);

        match op {
            Operation::Copy(from, to) => {
                info!("Worker: Copy {:?} -> {:?}", from, to);
                let res = copy_file(&from, &to);
                results.send(OpResult::Copied(res))?;
            }
            Operation::End => {
                info!("Worker received shutdown command.");
                break;
            }
        }

    }
    debug!("Worker {:?} shutting down", thread::current().id());
    Ok(())
}


pub fn copy_tree(opts: &Opts) -> Result<()> {
    let sourcedir = opts
        .source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource {
            msg: "Failed to find source directory name.",
        })?;
    let basedir = opts.dest.join(sourcedir);

    let (work_tx, work_rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();
    let _worker = thread::spawn(move || copy_worker(work_rx, res_tx));

    for entry in WalkDir::new(&opts.source).into_iter() {
        debug!("Got tree entry {:?}", entry);
        let from = entry?;
        let meta = from.metadata()?;
        let path = from.path().strip_prefix(&opts.source)?;
        let target = basedir.join(&path);

        match meta.file_type().to_enum() {
            FileType::File => {
                info!("Send copy operation {:?} to {:?}", from.path(), target);
                work_tx.send(Operation::Copy(from.path().to_path_buf(), target))?;
            },

            FileType::Dir => {
                info!("Creating directory: {:?}", target);
                create_dir(target)?;
            },

            FileType::Symlink => {
            },
        };
    }
    work_tx.send(Operation::End)?;

    for n in res_rx {
        println!("hi number {:?} from the main thread!", n);
    }
    println!("Ran out of results.");

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
