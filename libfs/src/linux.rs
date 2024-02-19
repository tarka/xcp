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

use std::{fs::File, path::Path};
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::PermissionsExt;

use linux_raw_sys::ioctl::{FS_IOC_FIEMAP, FIEMAP_EXTENT_LAST, FICLONE, FIEMAP_EXTENT_SHARED};
use rustix::fs::CWD;
use rustix::{fs::{copy_file_range, seek, mknodat, FileType, Mode, RawMode, SeekFrom}, io::Errno};

use crate::Extent;
use crate::errors::Result;
use crate::common::{copy_bytes_uspace, copy_range_uspace};

// Wrapper for copy_file_range(2) that checks for non-fatal errors due
// to limitations of the syscall.
fn try_copy_file_range(
    infd: &File,
    in_off: Option<&mut u64>,
    outfd: &File,
    out_off: Option<&mut u64>,
    bytes: u64,
) -> Option<Result<usize>> {
    let cfr_ret = copy_file_range(infd, in_off, outfd, out_off, bytes as usize);

    match cfr_ret {
        Ok(retval) => {
            Some(Ok(retval))
        },
        Err(Errno::NOSYS) | Err(Errno::PERM) | Err(Errno::XDEV) => {
            None
        },
        Err(errno) => {
            Some(Err(errno.into()))
        },
    }
}

/// File copy operation that defers file offset tracking to the
/// underlying call.  On Linux this attempts to use
/// [copy_file_range](https://man7.org/linux/man-pages/man2/copy_file_range.2.html)
/// and falls back to user-space if that is not available.
pub fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<usize> {
    try_copy_file_range(infd, None, outfd, None, bytes)
        .unwrap_or_else(|| copy_bytes_uspace(infd, outfd, bytes as usize))
}

/// File copy operation that that copies a block at offset`off`.  On
/// Linux this attempts to use
/// [copy_file_range](https://man7.org/linux/man-pages/man2/copy_file_range.2.html)
/// and falls back to user-space if that is not available.
pub fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<usize> {
    let mut off_in = off as u64;
    let mut off_out = off as u64;
    try_copy_file_range(infd, Some(&mut off_in), outfd, Some(&mut off_out), bytes)
        .unwrap_or_else(|| copy_range_uspace(infd, outfd, bytes as usize, off as usize))
}

/// Guestimate if file is sparse; if it has less blocks that would be
/// expected for its stated size. This is the same test used by
/// coreutils `cp`.
// FIXME: Should work on *BSD?
pub fn probably_sparse(fd: &File) -> Result<bool> {
    use std::os::linux::fs::MetadataExt;
    const ST_NBLOCKSIZE: u64 = 512;
    let stat = fd.metadata()?;
    Ok(stat.st_blocks() < stat.st_size() / ST_NBLOCKSIZE)
}

#[derive(PartialEq, Debug)]
enum SeekOff {
    Offset(u64),
    EOF,
}

fn lseek(fd: &File, from: SeekFrom) -> Result<SeekOff> {
    match seek(fd, from) {
        Err(errno) if errno == Errno::NXIO => Ok(SeekOff::EOF),
        Err(err) => Err(err.into()),
        Ok(off) => Ok(SeekOff::Offset(off)),
    }
}

const FIEMAP_PAGE_SIZE: usize = 32;

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
    fm_extents: [FiemapExtent; FIEMAP_PAGE_SIZE], // Array of mapped extents (out)
}
impl FiemapReq {
    fn new() -> FiemapReq {
        FiemapReq {
            fm_start: 0,
            fm_length: u64::max_value(),
            fm_flags: 0,
            fm_mapped_extents: 0,
            fm_extent_count: FIEMAP_PAGE_SIZE as u32,
            fm_reserved: 0,
            fm_extents: [FiemapExtent::new(); FIEMAP_PAGE_SIZE],
        }
    }
}

fn fiemap(fd: &File, req: &FiemapReq) -> Result<bool> {
    // FIXME: Rustix has an IOCTL mini-framework but it's a little
    // tricky and is unsafe anyway. This is simpler for now.
    let req_ptr: *const FiemapReq = req;
    if unsafe { libc::ioctl(fd.as_raw_fd(), FS_IOC_FIEMAP as u64, req_ptr) } != 0 {
        let oserr = io::Error::last_os_error();
        if oserr.raw_os_error() == Some(libc::EOPNOTSUPP) {
            return Ok(false)
        }
        return Err(oserr.into());
    }

    Ok(true)
}

/// Attempt to retrieve a map of the underlying allocated extents for
/// a file. Will return [None] if the filesystem doesn't support
/// extents. On Linux this is the raw list from
/// [fiemap](https://docs.kernel.org/filesystems/fiemap.html). See
/// [merge_extents](super::merge_extents) for a tool to merge contiguous extents.
pub fn map_extents(fd: &File) -> Result<Option<Vec<Extent>>> {
    let mut req = FiemapReq::new();
    let mut extents = Vec::with_capacity(FIEMAP_PAGE_SIZE);

    loop {
        if !fiemap(fd, &req)? {
            return Ok(None)
        }
        if req.fm_mapped_extents == 0 {
            break;
        }

        for i in 0..req.fm_mapped_extents as usize {
            let e = req.fm_extents[i];
            let ext = Extent {
                start: e.fe_logical,
                end: e.fe_logical + e.fe_length,
                shared: e.fe_flags & FIEMAP_EXTENT_SHARED != 0,
            };
            extents.push(ext);
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

/// Search the file for the next non-sparse file section. Returns the
/// start and end of the data segment.
// FIXME: Should work on *BSD too?
pub fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
    let next_data = match lseek(infd, SeekFrom::Data(pos as i64))? {
        SeekOff::Offset(off) => off,
        SeekOff::EOF => infd.metadata()?.len(),
    };
    let next_hole = match lseek(infd, SeekFrom::Hole(next_data as i64))? {
        SeekOff::Offset(off) => off,
        SeekOff::EOF => infd.metadata()?.len(),
    };

    lseek(infd, SeekFrom::Start(next_data))?; // FIXME: EOF (but shouldn't happen)
    lseek(outfd, SeekFrom::Start(next_data))?;

    Ok((next_data, next_hole))
}

/// Copy data between files, looking for sparse blocks and skipping
/// them.
pub fn copy_sparse(infd: &File, outfd: &File) -> Result<u64> {
    let len = infd.metadata()?.len();

    let mut pos = 0;
    while pos < len {
        let (next_data, next_hole) = next_sparse_segments(infd, outfd, pos)?;

        let _written = copy_file_bytes(infd, outfd, next_hole - next_data)?;
        pos = next_hole;
    }

    Ok(len)
}

/// Create a clone of a special file (unix socket, char-device, etc.)
pub fn copy_node(src: &Path, dest: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = src.metadata()?;
    let rmode = RawMode::from(meta.permissions().mode());
    let mode = Mode::from_raw_mode(rmode);
    let ftype = FileType::from_raw_mode(rmode);
    let dev = meta.dev();

    mknodat(CWD, dest, ftype, mode, dev)?;
    Ok(())
}

/// Reflink a file. This will reuse the underlying data on disk for
/// the target file, utilising copy-on-write for any future
/// updates. Only certain filesystems support this; if not supported
/// the function returns `false`.
pub fn reflink(infd: &File, outfd: &File) -> Result<bool> {
    if unsafe { libc::ioctl(outfd.as_raw_fd(), FICLONE as u64, infd.as_raw_fd()) } != 0 {
        let oserr = io::Error::last_os_error();
        match oserr.raw_os_error() {
            Some(libc::EOPNOTSUPP)
                | Some(libc::EINVAL)
                | Some(libc::EXDEV)
                | Some(libc::ETXTBSY) =>
                return Ok(false),
            _ =>
                return  Err(oserr.into()),
        }
    }
    Ok(true)
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    use super::*;
    use crate::{allocate_file, copy_permissions};
    use std::env::{current_dir, var};
    use std::fs::{read, OpenOptions};
    use std::io::{self, Seek, Write};
    use std::iter;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::process::Command;
    use linux_raw_sys::ioctl::FIEMAP_EXTENT_SHARED;
    use log::warn;
    use rustix::fs::FileTypeExt;
    use tempfile::{tempdir_in, TempDir};

    fn tempdir() -> Result<TempDir> {
        // Force into local dir as /tmp might be tmpfs, which doesn't
        // support all VFS options (notably fiemap).
        Ok(tempdir_in(current_dir()?.join("../target"))?)
    }

    #[test]
    #[cfg_attr(feature = "test_no_reflink", ignore = "No FS support")]
    fn test_reflink() -> Result<()> {
        let dir = tempdir()?;
        let from = dir.path().join("file.bin");
        let to = dir.path().join("copy.bin");
        let size = 128 * 1024;

        {
            let mut fd: File = File::create(&from)?;
            let data = "X".repeat(size);
            write!(fd, "{}", data)?;
        }

        let from_fd = File::open(from)?;
        let to_fd = File::create(to)?;

        {
            let from_map = FiemapReq::new();
            assert!(fiemap(&from_fd, &from_map)?);
            assert!(from_map.fm_mapped_extents > 0);
            // Un-refed file, no shared extents
            assert!(from_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED == 0);
        }

        let worked = reflink(&from_fd, &to_fd)?;
        assert!(worked);

        {
            let from_map = FiemapReq::new();
            assert!(fiemap(&from_fd, &from_map)?);
            assert!(from_map.fm_mapped_extents > 0);

            let to_map = FiemapReq::new();
            assert!(fiemap(&to_fd, &to_map)?);
            assert!(to_map.fm_mapped_extents > 0);

            // Now both have shared extents
            assert_eq!(from_map.fm_mapped_extents, to_map.fm_mapped_extents);
            assert!(from_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED != 0);
            assert!(to_map.fm_extents[0].fe_flags & FIEMAP_EXTENT_SHARED != 0);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_detection_small_data() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let fd = File::open(&file)?;
            assert!(probably_sparse(&fd)?);
        }
        {
            let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
            write!(fd, "test")?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_detection_half() -> Result<()> {
        assert!(!probably_sparse(&File::open("Cargo.toml")?)?);

        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        {
            let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
            let s = "x".repeat(512*1024);
            fd.write(s.as_bytes())?;
            assert!(probably_sparse(&fd)?);
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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
            .args(["-s", "1M", file.to_str().unwrap()])
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
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let offset = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset as u64;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
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
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_copy_range_middle() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";
        let offset: usize = 512 * 1024;

        {
            let mut fd = File::create(&from)?;
            fd.seek(io::SeekFrom::Start(offset as u64))?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let copied =
                copy_file_offset(&infd, &outfd, data.len() as u64, offset as i64)?;
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
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
            assert_eq!(copied as usize, data.len());
        }

        assert!(probably_sparse(&File::open(&file)?)?);

        let off = lseek(&File::open(&file)?, SeekFrom::Data(0))?;
        assert_eq!(off, SeekOff::Offset(offset));

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_sparse_rust_seek() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let data = "c00lc0d3";

        {
            let mut fd = File::create(&file)?;
            write!(fd, "{}", data)?;

            fd.seek(io::SeekFrom::Start(1024 * 4096))?;
            write!(fd, "{}", data)?;

            fd.seek(io::SeekFrom::Start(4096 * 4096 - data.len() as u64))?;
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
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_lseek_no_data() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());
        assert!(probably_sparse(&File::open(&file)?)?);

        let fd = File::open(&file)?;
        let off = lseek(&fd, SeekFrom::Data(0))?;
        assert!(off == SeekOff::EOF);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
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
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_empty_extent() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
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
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_fetch() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");
        let from = dir.path().join("from.txt");
        let data = "test data";

        {
            let mut fd = File::create(&from)?;
            write!(fd, "{}", data)?;
        }

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let offset = 512 * 1024;
        {
            let infd = File::open(&from)?;
            let outfd: File = OpenOptions::new().write(true).append(false).open(&file)?;
            let mut off_in = 0;
            let mut off_out = offset;
            let copied = copy_file_range(
                &infd,
                Some(&mut off_in),
                &outfd,
                Some(&mut off_out),
                data.len(),
            )?;
            assert_eq!(copied as usize, data.len());
        }

        let fd = File::open(file)?;

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), 1);
        assert_eq!(extents[0].start, offset as u64);
        assert_eq!(extents[0].end, offset as u64 + 4 * 1024); // FIXME: Assume 4k blocks
        assert!(!extents[0].shared);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_fetch_many() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("sparse.bin");

        let out = Command::new("/usr/bin/truncate")
            .args(["-s", "1M", file.to_str().unwrap()])
            .output()?;
        assert!(out.status.success());

        let fsize = 1024 * 1024;
        // FIXME: Assumes 4k blocks
        let bsize = 4 * 1024;
        let block = iter::repeat(0xff_u8).take(bsize).collect::<Vec<u8>>();

        let mut fd = OpenOptions::new().write(true).append(false).open(&file)?;
        // Skip every-other block
        for off in (0..fsize).step_by(bsize * 2) {
            lseek(&fd, SeekFrom::Start(off))?;
            fd.write_all(block.as_slice())?;
        }

        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();
        assert_eq!(extents.len(), fsize as usize / bsize / 2);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_not_sparse() -> Result<()> {
        let dir = tempdir()?;
        let file = dir.path().join("file.bin");
        let size = 128 * 1024;

        {
            let mut fd: File = File::create(&file)?;
            let data = "X".repeat(size);
            write!(fd, "{}", data)?;
        }

        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_some());
        let extents = extents_p.unwrap();

        assert_eq!(1, extents.len());
        assert_eq!(0 as u64, extents[0].start);
        assert_eq!(size as u64, extents[0].end);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_extents", ignore = "No FS support")]
    fn test_extent_unsupported_fs() -> Result<()> {
        let file = "/proc/cpuinfo";
        let fd = File::open(file)?;
        let extents_p = map_extents(&fd)?;
        assert!(extents_p.is_none());

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sparse", ignore = "No FS support")]
    fn test_copy_file_sparse() -> Result<()> {
        let dir = tempdir()?;
        let from = dir.path().join("sparse.bin");
        let len = 32 * 1024 * 1024;

        {
            let fd = File::create(&from)?;
            allocate_file(&fd, len)?;
        }

        assert_eq!(len, from.metadata()?.len());
        assert!(probably_sparse(&File::open(&from)?)?);

        let to = dir.path().join("sparse.copy.bin");
        crate::copy_file(&from, &to)?;

        assert_eq!(len, to.metadata()?.len());
        assert!(probably_sparse(&File::open(&to)?)?);

        Ok(())
    }

    #[test]
    #[cfg_attr(feature = "test_no_sockets", ignore = "No FS support")]
    fn test_copy_socket() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.sock");
        let to = dir.path().join("to.sock");

        let _sock = UnixListener::bind(&from).unwrap();
        assert!(from.metadata().unwrap().file_type().is_socket());

        copy_node(&from, &to).unwrap();

        assert!(to.exists());
        assert!(to.metadata().unwrap().file_type().is_socket());
    }


    #[test]
    #[cfg_attr(feature = "test_no_acl", ignore = "No FS support")]
    fn test_copy_acl() -> Result<()> {
        use exacl::{getfacl, AclEntry, Perm, setfacl};

        let dir = tempdir()?;
        let from = dir.path().join("file.bin");
        let to = dir.path().join("copy.bin");
        let data = "X".repeat(1024);

        {
            let mut fd: File = File::create(&from)?;
            write!(fd, "{}", data)?;

            let mut fd: File = File::create(&to)?;
            write!(fd, "{}", data)?;
        }

        let acl = AclEntry::allow_user("mail", Perm::READ, None);

        let mut from_acl = getfacl(&from, None)?;
        from_acl.push(acl.clone());
        setfacl(&[&from], &from_acl, None)?;

        {
            let from_fd: File = File::open(&from)?;
            let to_fd: File = File::open(&to)?;
            copy_permissions(&from_fd, &to_fd)?;
        }

        let to_acl = getfacl(&from, None)?;
        assert!(to_acl.contains(&acl));

        Ok(())
    }
}
