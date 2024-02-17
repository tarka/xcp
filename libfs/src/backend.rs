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
