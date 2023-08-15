/*
 * Copyright © 2018-2019, Steve Smith <tarkasteve@gmail.com>
 *
 * This program is free software: you can redistribute it and/or
 * modify it under the terms of the GNU General Public License version
 * 3 as published by the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use libc;
use std::cell::RefCell;
use std::fs::File;
use std::io;
use std::ops::Range;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::ptr;

use crate::errors::Result;
use crate::os::common::{copy_bytes_uspace, copy_range_uspace};

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
            off_in: *mut libc::loff_t,
            fd_out: libc::c_int,
            off_out: *mut libc::loff_t,
            len: libc::size_t,
            flags: libc::c_uint,
        ) -> libc::ssize_t;
    }
}

macro_rules! box_ptr_or_null(
    ($e:expr) =>
        (match $e {
            Some(b) =>  Box::into_raw(Box::new(b)),
            None => ptr::null_mut()
        })
);

// Kernels prior to 4.5 don't have copy_file_range, and it may not
// work across fs mounts, so we store the fallback to userspace in a
// thread-local flag to avoid unnecessary syscalls.
thread_local! {
    static USE_CFR: RefCell<bool> = RefCell::new(true);
}

fn copy_file_range(
    infd: &File,
    in_off: Option<i64>,
    outfd: &File,
    out_off: Option<i64>,
    bytes: u64,
) -> Option<Result<u64>> {
    USE_CFR.with(|cfr| {
        if *cfr.borrow() {
            // copy_file_range(2) takes an optional pointer to an
            // offset. However, taking pointers to the argument values
            // interacts badly with the optimiser and can end up
            // pointing at invalid values. To prevent this we force
            // the values onto the heap and take a pointer to
            // that. Note the cleanup below.
            let in_ptr = box_ptr_or_null!(in_off);
            let out_ptr = box_ptr_or_null!(out_off);

            let r = unsafe {
                ffi::copy_file_range(
                    infd.as_raw_fd(),
                    in_ptr,
                    outfd.as_raw_fd(),
                    out_ptr,
                    bytes as usize,
                    0,
                ) as i64
            };

            // Clean-up the allocated memory by pulling it back into a Box.
            if !in_ptr.is_null() {
                unsafe { Box::from_raw(in_ptr) };
            }
            if !out_ptr.is_null() {
                unsafe { Box::from_raw(out_ptr) };
            }

            match r {
                -1 => {
                    let errno = io::Error::last_os_error();
                    match errno.raw_os_error() {
                        Some(libc::ENOSYS) | Some(libc::EPERM) | Some(libc::EXDEV) => {
                            // Flag as unavailable and fallback to userspace.
                            *cfr.borrow_mut() = false;
                            None
                        }
                        _ => Some(Err(errno.into())),
                    }
                }
                _ => Some(Ok(r as u64)),
            }
        } else {
            None
        }
    })
}

// Wrapper for copy_file_range(2) that defers file offset tracking to
// the underlying call. See the manpage for details.
fn copy_bytes_kernel(infd: &File, outfd: &File, nbytes: u64) -> Option<Result<u64>> {
    copy_file_range(infd, None, outfd, None, nbytes)
}

// Wrapper for copy_file_range(2) that copies to the same position in
// the target file.
#[allow(dead_code)]
fn copy_range_kernel(infd: &File, outfd: &File, nbytes: u64, off: i64) -> Option<Result<u64>> {
    copy_file_range(infd, Some(off), outfd, Some(off), nbytes)
}

// Wrapper for copy_bytes_kernel that falls back to userspace if
// copy_file_range is not available.
pub fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
    copy_bytes_kernel(infd, outfd, bytes)
        .unwrap_or_else(|| copy_bytes_uspace(infd, outfd, bytes as usize))
}

// Wrapper for copy_range_kernel that copies a block . Falls back to userspace if
// copy_file_range is not available.
#[allow(dead_code)]
pub fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<u64> {
    copy_range_kernel(infd, outfd, bytes, off)
        .unwrap_or_else(|| copy_range_uspace(infd, outfd, bytes as usize, off as usize))
}

// Guestimate if file is sparse; if it has less blocks that would be
// expected for its stated size. This is the same test used by
// coreutils `cp`.
pub fn probably_sparse(fd: &File) -> Result<bool> {
    let stat = fd.metadata()?;
    Ok(stat.st_blocks() < stat.st_size() / stat.st_blksize())
}

/// Corresponds to lseek(2) `whence`
#[allow(dead_code)]
pub enum Whence {
    Set = libc::SEEK_SET as isize,
    Cur = libc::SEEK_CUR as isize,
    End = libc::SEEK_END as isize,
    Data = libc::SEEK_DATA as isize,
    Hole = libc::SEEK_HOLE as isize,
}

#[derive(PartialEq, Debug)]
pub enum SeekOff {
    Offset(u64),
    EOF,
}

pub fn lseek(fd: &File, off: i64, whence: Whence) -> Result<SeekOff> {
    let r = unsafe { libc::lseek64(fd.as_raw_fd(), off, whence as libc::c_int) };

    if r == -1 {
        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(errno) if errno == libc::ENXIO => Ok(SeekOff::EOF),
            _ => Err(err.into()),
        }
    } else {
        Ok(SeekOff::Offset(r as u64))
    }
}

// See ioctl_list(2)
#[allow(unused)]
const FS_IOC_FIEMAP: libc::c_ulong = 0xC020660B;
#[allow(unused)]
const FIEMAP_EXTENT_LAST: u32 = 0x00000001;
const PAGE_SIZE: usize = 32;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct FiemapExtent {
    fe_logical: u64,  // Logical offset in bytes for the start of the extent
    fe_physical: u64, // Physical offset in bytes for the start of the extent
    fe_length: u64,   // Length in bytes for the extent
    fe_reserved64: [u64; 2],
    fe_flags: u32, // FIEMAP_EXTENT_* flags for this extent
    fe_reserved: [u32; 3],
}
#[allow(unused)]
impl FiemapExtent {
    fn new() -> FiemapExtent {
        FiemapExtent {
            fe_logical: 0,
            fe_physical: 0,
            fe_length: 0,
            fe_reserved64: [0; 2],
            fe_flags: 0,
            fe_reserved: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct FiemapReq {
    fm_start: u64,          // Logical offset (inclusive) at which to start mapping (in)
    fm_length: u64,         // Logical length of mapping which userspace cares about (in)
    fm_flags: u32,          // FIEMAP_FLAG_* flags for request (in/out)
    fm_mapped_extents: u32, // Number of extents that were mapped (out)
    fm_extent_count: u32,   // Size of fm_extents array (in)
    fm_reserved: u32,
    fm_extents: [FiemapExtent; PAGE_SIZE], // Array of mapped extents (out)
}
#[allow(unused)]
impl FiemapReq {
    fn new() -> FiemapReq {
        FiemapReq {
            fm_start: 0,
            fm_length: u64::max_value(),
            fm_flags: 0,
            fm_mapped_extents: 0,
            fm_extent_count: PAGE_SIZE as u32,
            fm_reserved: 0,
            fm_extents: [FiemapExtent::new(); PAGE_SIZE],
        }
    }
}

#[allow(unused)]
pub fn map_extents(fd: &File) -> Result<Option<Vec<Range<u64>>>> {
    let mut req = FiemapReq::new();
    let req_ptr: *const FiemapReq = &req;
    let mut extents = Vec::with_capacity(PAGE_SIZE);

    loop {
        if unsafe { libc::ioctl(fd.as_raw_fd(), FS_IOC_FIEMAP, req_ptr) } != 0 {
            let oserr = io::Error::last_os_error();
            if oserr.raw_os_error() == Some(95) {
                return Ok(None)
            }
            return Err(oserr.into());
        }
        if req.fm_mapped_extents == 0 {
            break;
        }

        for i in 0..req.fm_mapped_extents as usize {
            let e = req.fm_extents[i];
            let start = e.fe_logical;
            let end = start + e.fe_length - 1;
            extents.push(start..end);
        }

        let last = req.fm_extents[(req.fm_mapped_extents - 1) as usize];
        if last.fe_flags & FIEMAP_EXTENT_LAST != 0 {
            break;
        }

        // Looks like we're going around again...
        req.fm_start = last.fe_logical + last.fe_length;
    }

    Ok(Some(extents))
}

pub fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
    let next_data = match lseek(infd, pos as i64, Whence::Data)? {
        SeekOff::Offset(off) => off,
        SeekOff::EOF => infd.metadata()?.len(),
    };
    let next_hole = match lseek(infd, next_data as i64, Whence::Hole)? {
        SeekOff::Offset(off) => off,
        SeekOff::EOF => infd.metadata()?.len(),
    };

    lseek(infd, next_data as i64, Whence::Set)?; // FIXME: EOF (but shouldn't happen)
    lseek(outfd, next_data as i64, Whence::Set)?;

    Ok((next_data, next_hole))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::allocate_file;
    use std::env::{current_dir, var};
    use std::fs::{read, OpenOptions};
    use std::io::{Seek, SeekFrom, Write};
    use std::iter;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::{tempdir_in, TempDir};

    fn tempdir() -> Result<TempDir> {
        // Force into local dir as /tmp might be tmpfs, which doesn't
        // support all VFS options (notably fiemap).
        Ok(tempdir_in(current_dir()?.join("target"))?)
    }

    fn fs_supports_extents() -> bool {
        // See `.github/workflows/rust.yml`
        let unsupported = vec!["ext2", "ntfs", "fat", "zfs"];
        match var("XCP_TEST_FS") {
            Ok(fs) => {
                !unsupported.contains(&fs.as_str())
            },
            Err(_) => true // assume 'normal' linux environment.
        }
    }

    #[test]
    fn test_sparse_detection() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let fd = File::open(&file)?;
            assert!(probably_sparse(&fd)?);
        }
        {
            let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
            write!(fd, "{}", "test")?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    fn test_copy_bytes_sparse() -> Result<()> {
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

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
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

        let offset: usize = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied = copy_file_range(
                &infd,
                Some(0),
                &outfd,
                Some(offset as i64),
                data.len() as u64,
            )
            .unwrap()?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;

        assert!(bytes.len() == 1024 * 1024);
        assert!(bytes[offset] == b't');
        assert!(bytes[offset + 1] == b'e');
        assert!(bytes[offset + 2] == b's');
        assert!(bytes[offset + 3] == b't');
        assert!(bytes[offset + data.len()] == 0);

        Ok(())
    }

    #[test]
    fn test_copy_range_middle() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset: usize = 512 * 1024;

        {
            let mut fd = File::create(&from)?;
            fd.seek(SeekFrom::Start(offset as u64))?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied =
                copy_range_kernel(&infd, &outfd, data.len() as u64, offset as i64).unwrap()?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert_eq!(bytes.len(), 1024 * 1024);
        assert_eq!(bytes[offset], b't');
        assert_eq!(bytes[offset + 1], b'e');
        assert_eq!(bytes[offset + 2], b's');
        assert_eq!(bytes[offset + 3], b't');
        assert_eq!(bytes[offset + data.len()], 0);

        Ok(())
    }

    #[test]
    fn test_lseek_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset = 512 * 1024;

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
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied = copy_file_range(
                &infd,
                Some(0),
                &outfd,
                Some(offset as i64),
                data.len() as u64,
            )
            .unwrap()?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let off = lseek(&File::open(&file)?, 0, Whence::Data)?;
        assert_eq!(off, SeekOff::Offset(offset));

        Ok(())
    }

    #[test]
    fn test_sparse_rust_seek() -> Result<()> {
        let dir = PathBuf::from("target");
        let file = dir.join("sparse.bin");

        let data = "c00lc0d3";

        {
            let mut fd = File::create(&file)?;
            write!(fd, "{}", data)?;

            fd.seek(SeekFrom::Start(1024 * 4096))?;
            write!(fd, "{}", data)?;

            fd.seek(SeekFrom::Start(4096 * 4096 - data.len() as u64))?;
            write!(fd, "{}", data)?;
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let bytes = read(&file)?;
        assert!(bytes.len() == 4096 * 4096);

        let offset = 1024 * 4096;
        assert!(bytes[offset] == b'c');
        assert!(bytes[offset + 1] == b'0');
        assert!(bytes[offset + 2] == b'0');
        assert!(bytes[offset + 3] == b'l');
        assert!(bytes[offset + data.len()] == 0);

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
        let off = lseek(&fd, 0, Whence::Data)?;
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

    #[test]
    fn test_empty_extent() -> Result<()> {
        if !fs_supports_extents() {
            return Ok(())
        }
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let fd = File::open(file)?;

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), 0);

        Ok(())
    }

    #[test]
    fn test_extent_fetch() -> Result<()> {
        if !fs_supports_extents() {
            return Ok(())
        }
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

        let offset: usize = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied = copy_file_range(
                &infd,
                Some(0),
                &outfd,
                Some(offset as i64),
                data.len() as u64,
            )
            .unwrap()?;
            assert_eq!(copied as usize, data.len());
        }

        let fd = File::open(file)?;

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), 1);
        assert_eq!(extents[0].start, offset as u64);
        assert_eq!(extents[0].end, offset as u64 + 4 * 1024 - 1); // FIXME: Assume 4k blocks

        Ok(())
    }

    #[test]
    fn test_extent_fetch_many() -> Result<()> {
        if !fs_supports_extents() {
            return Ok(())
        }
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(&["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let fsize = 1024 * 1024;
        // FIXME: Assumes 4k blocks
        let bsize = 4 * 1024;
        let block = iter::repeat(0xff as u8).take(bsize).collect::<Vec<u8>>();

        let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
        // Skip every-other block
        for off in (0..fsize).step_by(bsize * 2) {
            lseek(&fd, off, Whence::Set)?;
            fd.write_all(block.as_slice())?;
        }

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), fsize as usize / bsize / 2);

        Ok(())
    }

    #[test]
    fn test_extent_not_sparse() -> Result<()> {
        if !fs_supports_extents() {
            return Ok(())
        }
        let dir = tempdir()?;
        let file = dir.path().join("file.bin");
        let size = 128 * 1024;

        {
            let mut fd: File = File::create(&file)?;
            let data = iter::repeat("X").take(size).collect::<String>();
            write!(fd, "{}", data)?;
        }

        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();

        assert_eq!(1, extents.len());
        assert_eq!(0..size as u64 - 1, extents[0]);

        Ok(())
    }

    #[test]
    fn test_extent_unsupported_fs() -> Result<()> {
        if !fs_supports_extents() {
            return Ok(())
        }
        let file = "/proc/cpuinfo";
        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_none());

        Ok(())
    }
}
