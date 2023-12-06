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

use std::cmp;
use std::fs::{File, Metadata};
use std::path::Path;

use libfs::{
    allocate_file, copy_file_bytes, copy_permissions, next_sparse_segments, probably_sparse,
};

use crate::errors::Result;
use crate::options::Opts;
use crate::progress::{BatchUpdater, Updater};

#[derive(Debug)]
pub struct CopyHandle {
    pub infd: File,
    pub outfd: File,
    pub metadata: Metadata,
}

pub fn init_copy(from: &Path, to: &Path, opts: &Opts) -> Result<CopyHandle> {
    let infd = File::open(from)?;
    let metadata = infd.metadata()?;

    let outfd = File::create(to)?;
    allocate_file(&outfd, metadata.len())?;

    let handle = CopyHandle {
        infd,
        outfd,
        metadata,
    };

    // FIXME: This should happen at the end of the file copy, but with
    // the parblock handler this may be tricky. This works in practice.
    if !opts.no_perms {
        copy_permissions(&handle.infd, &handle.outfd)?;
    }

    Ok(handle)
}

/// Copy len bytes from wherever the descriptor cursors are set.
pub fn copy_bytes(handle: &CopyHandle, len: u64, updates: &mut BatchUpdater) -> Result<u64> {
    let mut written = 0u64;
    while written < len {
        let bytes_to_copy = cmp::min(len - written, updates.batch_size);
        let result = copy_file_bytes(&handle.infd, &handle.outfd, bytes_to_copy)? as u64;
        written += result;
        updates.update(Ok(result))?;
    }

    Ok(written)
}

/// Wrapper around copy_bytes that looks for sparse blocks and skips them.
pub fn copy_sparse(handle: &CopyHandle, updates: &mut BatchUpdater) -> Result<u64> {
    let len = handle.metadata.len();
    let mut pos = 0;

    while pos < len {
        let (next_data, next_hole) = next_sparse_segments(&handle.infd, &handle.outfd, pos)?;

        let _written = copy_bytes(handle, next_hole - next_data, updates)?;
        pos = next_hole;
    }

    Ok(len)
}

pub fn copy_file(from: &Path, to: &Path, opts: &Opts, updates: &mut BatchUpdater) -> Result<u64> {
    let handle = init_copy(from, to, opts)?;
    let total = if probably_sparse(&handle.infd)? {
        copy_sparse(&handle, updates)?
    } else {
        copy_bytes(&handle, handle.metadata.len(), updates)?
    };

    Ok(total)
}
