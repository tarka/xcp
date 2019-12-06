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

use std::path::{Path};
use std::cmp;
use std::fs::{File};
use log::{debug};

use crate::errors::{Result};
use crate::options::Opts;
use crate::os::{allocate_file, copy_file_bytes, probably_sparse, next_sparse_segments};
use crate::progress::{BatchUpdater, Updater};


/// Copy len bytes from wherever the descriptor cursors are set.
pub fn copy_bytes(infd: &File, outfd: &File, len: u64, updates: &mut BatchUpdater) -> Result<u64> {
    let mut written = 0u64;
    while written < len {
        let bytes_to_copy = cmp::min(len - written, updates.batch_size);
        let result = copy_file_bytes(&infd, &outfd, bytes_to_copy)?;
        written += result;
        updates.update(Ok(result))?;
    }

    Ok(written)
}

/// Wrapper around copy_bytes that looks for sparse blocks and skips them.
pub fn copy_sparse(infd: &File, outfd: &File, updates: &mut BatchUpdater) -> Result<u64> {
    let len = infd.metadata()?.len();
    allocate_file(&outfd, len)?;

    let mut pos = 0;

    while pos < len {
        let (next_data, next_hole) = next_sparse_segments(infd, outfd, pos)?;

        let _written = copy_bytes(infd, outfd, next_hole - next_data, updates)?;
        pos = next_hole;
    }

    Ok(len)
}


pub fn copy_file(from: &Path, to: &Path, _opts: &Opts, updates: &mut BatchUpdater) -> Result<u64> {
    let infd = File::open(from)?;
    let outfd = File::create(to)?;

    let total = if probably_sparse(&infd)? {
        debug!("File {:?} is sparse", from);
        copy_sparse(&infd, &outfd, updates)?

    } else {
        let len = infd.metadata()?.len();
        copy_bytes(&infd, &outfd, len, updates)?
    };

    outfd.set_permissions(infd.metadata()?.permissions())?;
    Ok(total)
}
