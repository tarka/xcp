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

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        mod linux;
        pub use common::{ allocate_file, merge_extents};
        pub use linux::{copy_file_bytes, copy_file_offset, probably_sparse, next_sparse_segments, map_extents};
    } else {
        pub use common::{allocate_file, copy_file_bytes, copy_file_offset, probably_sparse, next_sparse_segments, merge_extents, map_extents};
    }
}
