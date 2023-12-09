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
use std::sync::Arc;

use libfs::{
    allocate_file, copy_file_bytes, copy_permissions, next_sparse_segments, probably_sparse, sync,
};
use log::{debug, error};

use crate::errors::Result;
use crate::options::Opts;
use crate::progress::{BatchUpdater, Updater};

#[derive(Debug)]
pub struct CopyHandle {
    pub infd: File,
    pub outfd: File,
    pub metadata: Metadata,
    pub opts: Arc<Opts>,
}

impl CopyHandle {
    pub fn new(from: &Path, to: &Path, opts: Arc<Opts>) -> Result<CopyHandle> {
        let infd = File::open(from)?;
        let metadata = infd.metadata()?;

        let outfd = File::create(to)?;
        allocate_file(&outfd, metadata.len())?;

        let handle = CopyHandle {
            infd,
            outfd,
            metadata,
            opts: opts.clone(),
        };

        Ok(handle)
    }

    /// Copy len bytes from wherever the descriptor cursors are set.
    fn copy_bytes(&self, len: u64, updates: &mut BatchUpdater) -> Result<u64> {
        let mut written = 0u64;
        while written < len {
            let bytes_to_copy = cmp::min(len - written, updates.batch_size);
            let result = copy_file_bytes(&self.infd, &self.outfd, bytes_to_copy)? as u64;
            written += result;
            updates.update(Ok(result))?;
        }

        Ok(written)
    }

    /// Wrapper around copy_bytes that looks for sparse blocks and skips them.
    fn copy_sparse(&self, updates: &mut BatchUpdater) -> Result<u64> {
        let len = self.metadata.len();
        let mut pos = 0;

        while pos < len {
            let (next_data, next_hole) = next_sparse_segments(&self.infd, &self.outfd, pos)?;

            let _written = self.copy_bytes(next_hole - next_data, updates)?;
            pos = next_hole;
        }

        Ok(len)
    }

    pub fn copy_file(&self, updates: &mut BatchUpdater) -> Result<u64> {
        let total = if probably_sparse(&self.infd)? {
            self.copy_sparse(updates)?
        } else {
            self.copy_bytes(self.metadata.len(), updates)?
        };

        Ok(total)
    }

    fn finalise_copy(&self) -> Result<()> {
        if !self.opts.no_perms {
            copy_permissions(&self.infd, &self.outfd)?;
        }
        if self.opts.fsync {
            debug!("Syncing file {:?}", self.outfd);
            sync(&self.outfd)?;
        }
        Ok(())
    }
}

impl Drop for CopyHandle {
    fn drop(&mut self) {
        if let Err(e) = self.finalise_copy() {
            error!("Error during finalising copy operation {:?} -> {:?}: {}", self.infd, self.outfd, e);
        }
    }
}
