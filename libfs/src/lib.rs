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

use cfg_if::cfg_if;

mod attribs;
mod backend;
mod common;
mod errors;
mod fallback;
mod linux;

pub use attribs::{Extent, FileType, XATTR_SUPPORTED};
pub use errors::Error;

cfg_if! {
    if #[cfg(all(target_os = "linux", feature = "use_linux"))] {
        use linux as os_impl;
    } else {
        use fallback as os_impl;
    }
}
pub use os_impl::{
    copy_file_bytes,
    copy_file_offset,
    copy_node,
    copy_sparse,
    probably_sparse,
    next_sparse_segments,
    map_extents,
    reflink,
};
pub use common::{
    allocate_file,
    copy_file,
    copy_permissions,
    copy_timestamps,
    is_same_file,
    merge_extents,
    sync,
};
