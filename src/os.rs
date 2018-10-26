use libc;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::mem;
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::ptr::null_mut;

use crate::errors::Result;

/* **** Low level operations **** */

mod ffi {
    // Assumes Linux kernel >= 4.5.
    #[cfg(feature = "kernel_copy_file_range")]
    pub unsafe fn copy_file_range(
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
        pub fn copy_file_range(
            fd_in: libc::c_int,
            off_in: libc::loff_t,
            fd_out: libc::c_int,
            off_out: libc::loff_t,
            len: libc::size_t,
            flags: libc::c_uint,
        ) -> libc::ssize_t;
    }
}

/// Full mapping of copy_file_range(2). Not used directly, as we
/// always want to copy the same range to the same offset. See
/// wrappers below.
pub fn copy_file_range(infd: &File, mut in_off: i64,
                       outfd: &File, mut out_off: i64,
                       bytes: u64) -> Result<u64>
{
    let r = unsafe {
        ffi::copy_file_range(
            infd.as_raw_fd(),
            &mut in_off as *mut i64,
            outfd.as_raw_fd(),
            &mut out_off as *mut i64,
            bytes as usize,
            0,
        )
    };
    match r {
        -1 => Err(io::Error::last_os_error().into()),
        _ => Ok(r as u64),
    }
}

/// Version of copy_file_range(2) that copies the give range to the
/// same place in the target file.
pub fn copy_file_chunk(infd: &File, outfd: &File,
                       off: i64, bytes: u64) -> Result<u64>
{
    copy_file_range(infd, off, outfd, off, bytes)
}

/// Version of copy_file_range that defers offset-management to the
/// syscall. see copy_file_range(2) for details.
pub fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
    let r = unsafe {
        ffi::copy_file_range(
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

pub fn stat(path: &Path) -> Result<libc::stat> {
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
pub fn probably_sparse(fd: &Path) -> Result<bool> {
    let st = stat(fd)?;
    Ok(st.st_blocks < st.st_size / st.st_blksize)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{read, OpenOptions};
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
            copy_file_bytes(&infd, &outfd, data.len() as u64).unwrap();
        }

        assert!(probably_sparse(&file).unwrap());
    }

    #[test]
    fn test_sparse_copy_middle() {
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

        let offset: usize = 512*1024;
        {
            let infd = File::open(&from).unwrap();
            let outfd: File = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file).unwrap();
            copy_file_range(&infd, 0,
                            &outfd, offset as i64,
                            data.len() as u64).unwrap();
        }

        assert!(probably_sparse(&file).unwrap());

        let bytes = read(&file).unwrap();
        assert!(bytes.len() == 1024*1024);
        assert!(bytes[offset] == b't');
        assert!(bytes[offset+1] == b'e');
        assert!(bytes[offset+2] == b's');
        assert!(bytes[offset+3] == b't');
        assert!(bytes[offset+data.len()] == 0);
    }

}