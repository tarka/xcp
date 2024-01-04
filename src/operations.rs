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
use std::result;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel as cbc;
use libfs::{
    allocate_file, copy_file_bytes, copy_permissions, next_sparse_segments, probably_sparse, sync, reflink,
};
use log::{debug, error};

use crate::errors::{Result, XcpError};
use crate::options::Opts;
use crate::progress::{BatchUpdater, Updater};

#[derive(Clone, Debug, PartialEq)]
pub enum Reflink {
    Always,
    Auto,
    Never,
}

impl FromStr for Reflink {
    type Err = XcpError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "always" => Ok(Reflink::Always),
            "auto" => Ok(Reflink::Auto),
            "never" => Ok(Reflink::Never),
            _ => Err(XcpError::InvalidArguments(format!("Unexpected value for 'reflink': {}", s))),
        }
    }
}


#[derive(Debug)]
pub struct CopyHandle {
    pub infd: File,
    pub outfd: File,
    pub metadata: Metadata,
    pub opts: Arc<Opts>,
}

impl CopyHandle {
    pub fn new(from: &Path, to: &Path, opts: &Arc<Opts>) -> Result<CopyHandle> {
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
    fn copy_bytes(&self, len: u64, updates: &StatSender) -> Result<u64> {
        let mut written = 0u64;
        while written < len {
            let bytes_to_copy = cmp::min(len - written, self.opts.batch_size());
            let result = copy_file_bytes(&self.infd, &self.outfd, bytes_to_copy)? as u64;
            written += result;
            updates.send(StatusUpdate::Copied(result as u64), bytes_to_copy, self.opts.block_size)?;
        }

        Ok(written)
    }

    /// Wrapper around copy_bytes that looks for sparse blocks and skips them.
    fn copy_sparse(&self, updates: &StatSender) -> Result<u64> {
        let len = self.metadata.len();
        let mut pos = 0;

        while pos < len {
            let (next_data, next_hole) = next_sparse_segments(&self.infd, &self.outfd, pos)?;

            let _written = self.copy_bytes(next_hole - next_data, updates)?;
            pos = next_hole;
        }

        Ok(len)
    }

    pub fn try_reflink(&self) -> Result<bool> {
        match self.opts.reflink {
            Reflink::Always | Reflink::Auto => {
                debug!("Attempting reflink from {:?}->{:?}", self.infd, self.outfd);
                let worked = reflink(&self.infd, &self.outfd)?;
                if worked {
                    debug!("Reflink {:?} succeeded", self.outfd);
                    return Ok(true)
                } else if self.opts.reflink == Reflink::Always {
                    return Err(XcpError::ReflinkFailed(format!("{:?}->{:?}", self.infd, self.outfd)).into());
                } else {
                    debug!("Failed to reflink, falling back to copy");
                    Ok(false)
                }
            }

            Reflink::Never => {
                Ok(false)
            }
        }
    }

    pub fn copy_file(&self, updates: &StatSender) -> Result<u64> {
        if self.try_reflink()? {
            return Ok(self.metadata.len());
        }
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
        // FIXME: SHould we chcek for panicking() here?
        if let Err(e) = self.finalise_copy() {
            error!("Error during finalising copy operation {:?} -> {:?}: {}", self.infd, self.outfd, e);
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    Copied(u64),
    Size(u64),
}

impl StatusUpdate {
    /// Return new instance of enum with updated value
    pub fn set(&self, bytes: u64) -> StatusUpdate {
        match self {
            StatusUpdate::Copied(_) => StatusUpdate::Copied(bytes),
            StatusUpdate::Size(_) => StatusUpdate::Size(bytes),
        }
    }
    /// Extract the value of enum.
    pub fn value(&self) -> u64 {
        match self {
            StatusUpdate::Copied(v) => *v,
            StatusUpdate::Size(v) => *v,
        }
    }
}

// FIXME: We should probably abstract away more of the channel setup
// to be no-ops when --no-progress is specified.
static BYTE_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct StatSender {
    noop: bool,
    chan: cbc::Sender<StatusUpdate>,
}
impl StatSender {
    pub fn new(chan: cbc::Sender<StatusUpdate>, opts: &Opts) -> StatSender {
        StatSender {
            noop: opts.no_progress,
            chan,
        }
    }

    // Wrapper around channel-send that groups updates together
    pub fn send(&self, update: StatusUpdate, bytes: u64, bsize: u64) -> Result<()> {
        if self.noop {
            return Ok(());
        }
        // Avoid saturating the queue with small writes
        let prev_written = BYTE_COUNT.fetch_add(bytes, Ordering::Relaxed);
        if ((prev_written + bytes) / bsize) > (prev_written / bsize) {
            Ok(self.chan.send(update)?)
        } else {
            Ok(())
        }
    }
}
