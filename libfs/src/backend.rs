/*
 * Copyright © 2024, Steve Smith <tarkasteve@gmail.com>
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

use crate::attribs::Extent;
use crate::errors::Result;

pub trait Backend {
    fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<usize>;
    fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<usize>;
    fn probably_sparse(fd: &File) -> Result<bool>;
    fn map_extents(fd: &File) -> Result<Option<Vec<Extent>>>;
    fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)>;
    fn copy_sparse(infd: &File, outfd: &File) -> Result<u64>;
    fn copy_node(src: &Path, dest: &Path) -> Result<()>;
    fn reflink(infd: &File, outfd: &File) -> Result<bool>;
    fn copy_permissions(infd: &File, outfd: &File) -> Result<()>;
    fn copy_timestamps(infd: &File, outfd: &File) -> Result<()>;
    fn allocate_file(fd: &File, len: u64) -> Result<()>;
    fn merge_extents(extents: Vec<Extent>) -> Result<Vec<Extent>>;
    fn is_same_file(src: &Path, dest: &Path) -> Result<bool>;
    fn copy_file(from: &Path, to: &Path) -> Result<u64>;
    fn sync(fd: &File) -> Result<()>;
}

struct LinuxBackend;
struct UspaceBackend;

use crate::{common, fallback, linux};

impl Backend for LinuxBackend {
    fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<usize> {
        linux::copy_file_bytes(infd, outfd, bytes)
    }
    fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<usize> {
        linux::copy_file_offset(infd, outfd, bytes, off)
    }
    fn probably_sparse(fd: &File) -> Result<bool> {
        linux::probably_sparse(fd)
    }
    fn map_extents(fd: &File) -> Result<Option<Vec<Extent>>> {
        linux::map_extents(fd)
    }
    fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
        linux::next_sparse_segments(infd, outfd, pos)
    }
    fn copy_sparse(infd: &File, outfd: &File) -> Result<u64> {
        linux::copy_sparse(infd, outfd)
    }
    fn copy_node(src: &Path, dest: &Path) -> Result<()> {
        linux::copy_node(src, dest)
    }
    fn reflink(infd: &File, outfd: &File) -> Result<bool> {
        linux::reflink(infd, outfd)
    }
    fn copy_permissions(infd: &File, outfd: &File) -> Result<()> {
        common::copy_permissions(infd, outfd)
    }
    fn copy_timestamps(infd: &File, outfd: &File) -> Result<()> {
        common::copy_timestamps(infd, outfd)
    }
    fn allocate_file(fd: &File, len: u64) -> Result<()> {
        common::allocate_file(fd, len)
    }
    fn merge_extents(extents: Vec<Extent>) -> Result<Vec<Extent>> {
        common::merge_extents(extents)
    }
    fn is_same_file(src: &Path, dest: &Path) -> Result<bool> {
        common::is_same_file(src, dest)
    }
    fn copy_file(from: &Path, to: &Path) -> Result<u64> {
        common::copy_file(from, to)
    }
    fn sync(fd: &File) -> Result<()> {
        common::sync(fd)
    }
}

impl Backend for UspaceBackend {
    fn copy_file_bytes(infd: &File, outfd: &File, bytes: u64) -> Result<usize> {
        fallback::copy_file_bytes(infd, outfd, bytes)
    }
    fn copy_file_offset(infd: &File, outfd: &File, bytes: u64, off: i64) -> Result<usize> {
        fallback::copy_file_offset(infd, outfd, bytes, off)
    }
    fn probably_sparse(fd: &File) -> Result<bool> {
        fallback::probably_sparse(fd)
    }
    fn map_extents(fd: &File) -> Result<Option<Vec<Extent>>> {
        fallback::map_extents(fd)
    }
    fn next_sparse_segments(infd: &File, outfd: &File, pos: u64) -> Result<(u64, u64)> {
        fallback::next_sparse_segments(infd, outfd, pos)
    }
    fn copy_sparse(infd: &File, outfd: &File) -> Result<u64> {
        fallback::copy_sparse(infd, outfd)
    }
    fn copy_node(src: &Path, dest: &Path) -> Result<()> {
        fallback::copy_node(src, dest)
    }
    fn reflink(infd: &File, outfd: &File) -> Result<bool> {
        fallback::reflink(infd, outfd)
    }
    fn copy_permissions(infd: &File, outfd: &File) -> Result<()> {
        common::copy_permissions(infd, outfd)
    }
    fn copy_timestamps(infd: &File, outfd: &File) -> Result<()> {
        common::copy_timestamps(infd, outfd)
    }
    fn allocate_file(fd: &File, len: u64) -> Result<()> {
        common::allocate_file(fd, len)
    }
    fn merge_extents(extents: Vec<Extent>) -> Result<Vec<Extent>> {
        common::merge_extents(extents)
    }
    fn is_same_file(src: &Path, dest: &Path) -> Result<bool> {
        common::is_same_file(src, dest)
    }
    fn copy_file(from: &Path, to: &Path) -> Result<u64> {
        common::copy_file(from, to)
    }
    fn sync(fd: &File) -> Result<()> {
        common::sync(fd)
    }
}
