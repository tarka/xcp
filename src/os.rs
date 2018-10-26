use libc;
use std::fs::File;
use std::path::PathBuf;
use std::mem;
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::ptr::null_mut;

use crate::errors::Result;

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

pub fn r_copy_file_range(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
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


pub fn stat(path: &PathBuf) -> Result<libc::stat> {
    let pbytes = path.as_os_str().as_bytes();

    let mut stat: libc::stat = unsafe { mem::uninitialized() };
    let r = unsafe {
        let cstr: &[i8] = &*(pbytes as *const [u8] as *const [i8]);
        libc::stat(cstr.as_ptr(), &mut stat)
    };

    match r {
        -1 => Err(io::Error::last_os_error().into()),
        _ => Ok(stat),
    }
}


// Guestimate if file is sparse; if it has less blocks that would be
// expected for its stated size. This is the same test used by
// coreutils `cp`.
pub fn probably_sparse(fd: &PathBuf) -> Result<bool> {
    let st = stat(fd)?;
    Ok(st.st_blocks < st.st_size / st.st_blksize)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::OpenOptions;
    use std::process::Command;
    use std::io::Write;

    #[test]
    fn test_stat() {
        let hosts = PathBuf::from("/etc/hosts");
        let hsize = hosts.metadata().unwrap().len() as i64;
        let hstat = stat(&hosts).unwrap();
        assert!(hsize == hstat.st_size);
    }

    #[test]
    fn test_sparse_detection() {
        assert!(!probably_sparse(&PathBuf::from("Cargo.toml")).unwrap());

        let dir = tempdir().unwrap();
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success());

        assert!(probably_sparse(&file).unwrap());
        {
            let mut fd = File::open(&file).unwrap();
            write!(fd, "{}", "test");
        }
        assert!(probably_sparse(&file).unwrap());
    }

    #[test]
    fn test_copy_range_sparse() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from).unwrap();
            write!(fd, "{}", data);
        }

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success());

        {
            let infd = File::open(&from).unwrap();
            let outfd: File = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file).unwrap();
            r_copy_file_range(&infd, &outfd, 4).unwrap();
        }

        assert!(probably_sparse(&file).unwrap());
    }

}
