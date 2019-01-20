
use libc;
use std::fs::File;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::ptr::null_mut;

use crate::os::{SeekOff, Wence};
use crate::os::common::result_or_errno;
use crate::errors::Result;


/* **** Low level operations **** */

// Assumes Linux kernel >= 4.5.
mod ffi {
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
        ) as i64
    };
    result_or_errno(r, r as u64)
}


/// Full mapping of copy_file_range(2). Not used directly, as we
/// always want to copy the same range to the same offset. See
/// wrappers below.
#[allow(dead_code)]
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
        ) as i64
    };
    result_or_errno(r, r as u64)
}


pub fn allocate_file(fd: &File, len: u64) -> Result<()> {
    let r = unsafe {
        libc::ftruncate(fd.as_raw_fd(), len as i64)
    };
    result_or_errno(r as i64, ())
}


// Guestimate if file is sparse; if it has less blocks that would be
// expected for its stated size. This is the same test used by
// coreutils `cp`.
pub fn probably_sparse(fd: &File) -> Result<bool> {
    let stat = fd.metadata()?;
    Ok(stat.st_blocks() < stat.st_size() / stat.st_blksize())
}



pub fn lseek(fd: &File, off: i64, wence: Wence) -> Result<SeekOff> {
    let r = unsafe {
        libc::lseek64(
            fd.as_raw_fd(),
            off,
            wence as libc::c_int
        )
    };

    if r == -1 {
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(errno) if errno == libc::ENXIO => {
                Ok(SeekOff::EOF)
            }
            _ => Err(err.into())
        }

    } else {
        Ok(SeekOff::Offset(r as u64))
    }

}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::{PathBuf};
    use std::fs::{read, OpenOptions};
    use std::process::Command;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn test_sparse_detection() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()
            ?;
        assert!(out.status.success());

        {
            let fd = File::open(&file)?;
            assert!(probably_sparse(&fd)?);
        }
        {
            let mut fd = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file)?;
            write!(fd, "{}", "test")?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    fn test_copy_range_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()
            ?;
        assert!(out.status.success());

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file)?;
            copy_file_bytes(&infd, &outfd, data.len() as u64)?;
        }

        assert!(probably_sparse(&File::open(file)?)?);

        Ok(())
    }

    #[test]
    fn test_sparse_copy_middle() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let offset: usize = 512*1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file)?;
            copy_file_range(&infd, 0,
                            &outfd, offset as i64,
                            data.len() as u64)?;
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert!(bytes.len() == 1024*1024);
        assert!(bytes[offset] == b't');
        assert!(bytes[offset+1] == b'e');
        assert!(bytes[offset+2] == b's');
        assert!(bytes[offset+3] == b't');
        assert!(bytes[offset+data.len()] == 0);

        Ok(())
    }

    #[test]
    fn test_lseek_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset = 512*1024;

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new()
                .write(true)
                .append(false)
                .open(&file)?;
            copy_file_range(&infd, 0,
                            &outfd, offset as i64,
                            data.len() as u64)?;
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let off = lseek(&File::open(&file)?, 0, Wence::Data)?;
        assert_eq!(off, SeekOff::Offset(offset));

        Ok(())
    }

    #[test]
    fn test_sparse_rust_seek() -> Result<()> {
        //let dir = tempdir()?;
        let dir = PathBuf::from("target");
        let file = dir.join("sparse.bin");

        let data = "c00lc0d3";

        {
            let mut fd = File::create(&file)?;
            write!(fd, "{}", data)?;

            fd.seek(SeekFrom::Start(1024*4096))?;
            write!(fd, "{}", data)?;

            fd.seek(SeekFrom::Start(4096*4096 - data.len() as u64))?;
            write!(fd, "{}", data)?;
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert!(bytes.len() == 4096*4096);

        let offset = 1024 * 4096;
        assert!(bytes[offset] == b'c');
        assert!(bytes[offset+1] == b'0');
        assert!(bytes[offset+2] == b'0');
        assert!(bytes[offset+3] == b'l');
        assert!(bytes[offset+data.len()] == 0);

        Ok(())
    }


    #[test]
    fn test_lseek_no_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        assert!(probably_sparse(&File::open(&file)?)?);

        let fd = File::open(&file)?;
        let off = lseek(&fd, 0, Wence::Data)?;
        assert!(off == SeekOff::EOF);

        Ok(())
    }

    #[test]
    fn test_allocate_file_is_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let len = 32 * 1024 * 1024;

        {
            let fd = File::create(&file)?;
            allocate_file(&fd, len)?;
        }

        assert_eq!(len, file.metadata()?.len());
        assert!(probably_sparse(&File::open(&file)?)?);

        Ok(())
    }
}
