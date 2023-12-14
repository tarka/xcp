/*
 * Copyright © 2018, Steve Smith <tarkasteve@gmail.com>
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

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::ops::Range;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;

use libc::{dev_t, mode_t};

use crate::common::{copy_bytes_uspace, copy_range_uspace};
use crate::errors::{Result, Error};

pub fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<usize> {
    copy_bytes_uspace(infd, outfd, bytes as usize)
}

pub fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<usize> {
    copy_range_uspace(infd, outfd, bytes as usize, off as usize)
}

// No sparse file handling by default, needs to be implemented
// per-OS. This effectively disables the following operations.
pub fn probably_sparse(_fd: &File) -> Result<bool> {
    Ok(false)
}

pub fn map_extents(_fd: &File) -> Result<Option<Vec<Range<u64>>>> {
    // FIXME: Implement for *BSD with lseek?
    Ok(None)
}

pub fn next_sparse_segments(_infd: &File, _outfd: &File, _pos: u64) -> Result<(u64, u64)> {
    // FIXME: Implement for *BSD with lseek?
    Err(Error::UnsupportedOperation {})
}

pub fn copy_sparse(infd: &File, outfd: &File) -> Result<u64> {
    let len = infd.metadata()?.len();
    copy_file_bytes(&infd, &outfd, len)
        .map(|i| i as u64)
}

pub fn copy_node(src: &Path, dest: &Path) -> Result<()> {
    let meta = src.metadata()?;
    let mode = meta.permissions().mode();
    let dev = meta.dev();
    let pstr = dest.to_str()
        .ok_or(Error::InvalidPath(dest.to_path_buf()))?;
    let cdest = CString::new(pstr)
        .map_err(|_| Error::InvalidPath(dest.to_path_buf()))?;

    if unsafe { libc::mknod(cdest.into_raw(), mode as mode_t, dev as dev_t) } != 0 {
        let errno = io::Error::last_os_error();
        return Err(errno.into())
    }
    Ok(())
}
