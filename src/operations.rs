use log::{debug, info};
use std::cmp;
use std::fs::{create_dir, create_dir_all, File};
use std::io;
use std::io::ErrorKind as IOKind;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc;
use threadpool::ThreadPool;
use walkdir::WalkDir;

use libc;

use crate::Opts;
use crate::errors::{io_err, Result, XcpError};
use crate::utils::{FileType, ToFileType};

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

pub fn copy_file(from: &Path, to: &Path) -> Result<u64> {
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


pub fn copy_tree(opts: &Opts) -> Result<()> {
    let sourcedir = opts
        .source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource {
            msg: "Failed to find source directory name.",
        })?;
    let basedir = opts.dest.join(sourcedir);

    for entry in WalkDir::new(&opts.source).into_iter() {
        debug!("Got tree entry {:?}", entry);
        let from = entry?;
        let meta = from.metadata()?;
        let path = from.path().strip_prefix(&opts.source)?;
        let target = basedir.join(&path);

        match meta.file_type().to_enum() {
            FileType::File => {
                info!("Copying file {:?} to {:?}", from.path(), target);
                copy_file(&from.path(), &target)?;
            },

            FileType::Dir => {
                info!("Creating directory: {:?}", target);
                create_dir(target)?;
            },

            FileType::Symlink => {
            },
        };

    }

    Ok(())
}


fn par_copy(from: PathBuf, to: PathBuf) {
    let dir = to.parent().unwrap(); // FIXME
    create_dir_all(&dir).unwrap();
    copy_file(&from, &to).unwrap();
}

pub fn par_copy_tree(opts: &Opts) -> Result<()> {
    let sourcedir = opts
        .source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource {
            msg: "Failed to find source directory name.",
        })?;
    let basedir = opts.dest.join(sourcedir);

    let pool = ThreadPool::new(2);
    //let (work_tx, work_rx) = mpsc::channel();

    for entry in WalkDir::new(&opts.source).into_iter() {
        debug!("[par] Got tree entry {:?}", entry);
        let from = entry?;
        let meta = from.metadata()?;
        let path = from.path().strip_prefix(&opts.source)?;
        let target = basedir.join(&path);

        match meta.file_type().to_enum() {
            FileType::File => {
                info!("[par] Copying file {:?} to {:?}", from.path(), target);
                pool.execute(move || par_copy(from.path().to_path_buf(), target));
            },

            FileType::Dir => {
                if from.path().read_dir()?.count() == 0 {
                    info!("[par] Creating directory: {:?}", target);
                    create_dir_all(target)?;
                }
            },

            FileType::Symlink => {
            },
        };
    }
    pool.join();

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
