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

mod common;

use std::fs::File;
use std::ops::Range;

use crate::options::Opts;
use crate::operations::CopyHandle;
use crate::errors::Result;

pub trait FsOperations {
    fn allocate_file(fd: &File, len: u64) -> Result<()>;
    fn copy_permissions(hdl: &CopyHandle, opts: &Opts) -> Result<()>;
    fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<u64>;
    fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<u64>;
    fn probably_sparse(fd: &File) -> Result<bool>;
    fn map_extents(fd: &File) -> Result<Vec<Range<u64>>>;
    fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)>;
}

struct Common;
impl FsOperations for Common {
    #[inline(always)]
    fn allocate_file(fd: &File, len: u64) -> Result<()> {
        common::allocate_file(fd, len)
    }

    #[inline(always)]
    fn copy_permissions(hdl: &CopyHandle, opts: &Opts) -> Result<()> {
        common::copy_permissions(hdl, opts)
    }

    #[inline(always)]
    fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
        common::copy_file_bytes(infd, outfd, bytes)
    }

    #[inline(always)]
    fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<u64> {
        common::copy_file_offset(infd, outfd, bytes, off)
    }

    #[inline(always)]
    fn probably_sparse(fd: &File) -> Result<bool> {
        common::probably_sparse(fd)
    }

    #[inline(always)]
    fn map_extents(fd: &File) -> Result<Vec<Range<u64>>> {
        common::map_extents(fd)
    }

    #[inline(always)]
    fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
        common::next_sparse_segments(infd, outfd, pos)
    }

}

cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        mod linux;

        struct Linux;
        impl FsOperations for Linux {
            #[inline(always)]
            fn allocate_file(fd: &File, len: u64) -> Result<()> {
                common::allocate_file(fd, len)
            }

            #[inline(always)]
            fn copy_permissions(hdl: &CopyHandle, opts: &Opts) -> Result<()> {
                common::copy_permissions(hdl, opts)
            }

            #[inline(always)]
            fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<u64> {
                linux::copy_file_bytes(infd, outfd, bytes)
            }

            #[inline(always)]
            fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<u64> {
                linux::copy_file_offset(infd, outfd, bytes, off)
            }

            #[inline(always)]
            fn probably_sparse(fd: &File) -> Result<bool> {
                linux::probably_sparse(fd)
            }

            #[inline(always)]
            fn map_extents(fd: &File) -> Result<Vec<Range<u64>>> {
                linux::map_extents(fd)
            }

            #[inline(always)]
            fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
                linux::next_sparse_segments(infd, outfd, pos)
            }

        }
    }
}


use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
//        mod linux;
        pub use common::{
            allocate_file,
            copy_permissions,
        };
        pub use linux::{
            copy_file_bytes,
            copy_file_offset,
            probably_sparse,
            next_sparse_segments,
            map_extents
        };

    } else {
        pub use common::{
            allocate_file,
            copy_file_bytes,
            copy_file_offset,
            copy_permissions,
            probably_sparse,
            next_sparse_segments,
            map_extents
        };
    }
}
