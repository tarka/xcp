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

use cfg_if::cfg_if;
use std::fs;
use std::path::{Path, PathBuf};

pub enum FileType {
    File,
    Dir,
    Symlink,
    Unknown,
}

pub trait ToFileType {
    fn to_enum(self) -> FileType;
}

fn to_enum(ft: fs::FileType) -> FileType {
    if ft.is_dir() {
        FileType::Dir
    } else if ft.is_file() {
        FileType::File
    } else if ft.is_symlink() {
        FileType::Symlink
    } else {
        FileType::Unknown
    }
}

impl ToFileType for fs::FileType {
    fn to_enum(self) -> FileType {
        to_enum(self)
    }
}

pub fn empty(path: &Path) -> bool {
    *path == PathBuf::new()
}

// NOTE: The xattr crate has a SUPPORTED_PLATFORM flag, however it
// allows NetBSD, which fails for us, so we stick to platforms we've
// tested.
pub fn xattr_supported() -> bool {
    cfg_if! {
        if #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))] {
            true
        } else {
            false
        }
    }
}
