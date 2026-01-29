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
pub(crate) enum SeekOff {
    Offset(u64),
    EOF,
}

pub(crate) fn lseek(fd: &File, from: SeekFrom) -> Result<SeekOff> {
    match seek(fd, from) {
        Err(errno) if errno == Errno::NXIO => Ok(SeekOff::EOF),
        Err(err) => Err(err.into()),
        Ok(off) => Ok(SeekOff::Offset(off)),
    }
}

const FIEMAP_PAGE_SIZE: usize = 32;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct FiemapExtent {
    pub(crate) fe_logical: u64,  // Logical offset in bytes for the start of the extent
    pub(crate) fe_physical: u64, // Physical offset in bytes for the start of the extent
    pub(crate) fe_length: u64,   // Length in bytes for the extent
    pub(crate) fe_reserved64: [u64; 2],
    pub(crate) fe_flags: u32, // FIEMAP_EXTENT_* flags for this extent
    pub(crate) fe_reserved: [u32; 3],
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
pub(crate) struct FiemapReq {
    pub(crate) fm_start: u64,          // Logical offset (inclusive) at which to start mapping (in)
    pub(crate) fm_length: u64,         // Logical length of mapping which userspace cares about (in)
    pub(crate) fm_flags: u32,          // FIEMAP_FLAG_* flags for request (in/out)
    pub(crate) fm_mapped_extents: u32, // Number of extents that were mapped (out)
    pub(crate) fm_extent_count: u32,   // Size of fm_extents array (in)
    pub(crate) fm_reserved: u32,
    pub(crate) fm_extents: [FiemapExtent; FIEMAP_PAGE_SIZE], // Array of mapped extents (out)
}
impl FiemapReq {
    pub(crate) fn new() -> FiemapReq {
        FiemapReq {
            fm_start: 0,
            fm_length: u64::MAX,
            fm_flags: 0,
            fm_mapped_extents: 0,
            fm_extent_count: FIEMAP_PAGE_SIZE as u32,
            fm_reserved: 0,
            fm_extents: [FiemapExtent::new(); FIEMAP_PAGE_SIZE],
        }
    }
}

pub(crate) fn fiemap(fd: &File, req: &mut FiemapReq) -> Result<bool> {
    // FIXME: Rustix has an IOCTL mini-framework but it's a little
    // tricky and is unsafe anyway. This is simpler for now.
    let req_ptr: *mut FiemapReq = req;
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
        if !fiemap(fd, &mut req)? {
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
    let next_data = match lseek(infd, SeekFrom::Data(pos))? {
        SeekOff::Offset(off) => off,
        SeekOff::EOF => infd.metadata()?.len(),
    };
    let next_hole = match lseek(infd, SeekFrom::Hole(next_data))? {
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
