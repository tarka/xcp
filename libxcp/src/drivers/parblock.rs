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

//! Parallelise copying at the block level. Block-size is
//! configurable. This can have better performance for large files,
//! but has a higher overhead.

use std::cmp;
use std::ops::Range;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use cfg_if::cfg_if;
use crossbeam_channel as cbc;
use libfs::copy_node;
use log::{error, info};
use blocking_threadpool::{Builder, ThreadPool};

use crate::config::Config;
use crate::drivers::CopyDriver;
use crate::errors::{Result, XcpError};
use crate::feedback::{StatusUpdate, StatusUpdater};
use crate::operations::{CopyHandle, Operation, tree_walker};
use libfs::{copy_file_offset, map_extents, merge_extents, probably_sparse};

// ********************************************************************** //

const fn supported_platform() -> bool {
    cfg_if! {
        if #[cfg(
            any(target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "dragonfly",
                target_os = "macos",
            ))]
        {
            true
        } else {
            false
        }
    }
}


pub struct Driver {
    config: Arc<Config>,
}

impl Driver {
    pub fn new(config: Arc<Config>) -> Result<Self> {
        if !supported_platform() {
            let msg = "The parblock driver is not currently supported on this OS.";
            error!("{}", msg);
            return Err(XcpError::UnsupportedOS(msg).into());
        }

        Ok(Self {
            config,
        })
    }
}

impl CopyDriver for Driver {
    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, stats: Arc<dyn StatusUpdater>) -> Result<()> {
        let (file_tx, file_rx) = cbc::unbounded::<Operation>();

        // Start (single) dispatch worker
        let dispatcher = {
            let q_config = self.config.clone();
            let st = stats.clone();
            thread::spawn(move || dispatch_worker(file_rx, &st, q_config))
        };

        // Thread which walks the file tree and sends jobs to the
        // workers. The worker tx channel is moved to the walker so it is
        // closed, which will cause the workers to shutdown on completion.
        let _walk_worker = {
            let sc = stats.clone();
            let d = dest.to_path_buf();
            let c = self.config.clone();
            thread::spawn(move || tree_walker(sources, &d, &c, file_tx, sc))
        };

        dispatcher.join()
            .map_err(|_| XcpError::CopyError("Error dispatching copy operation".to_string()))??;

        Ok(())
    }

    fn copy_single(&self, source: &Path, dest: &Path, stats: Arc<dyn StatusUpdater>) -> Result<()> {
        let nworkers = self.config.num_workers();
        let pool = ThreadPool::new(nworkers);

        queue_file_blocks(source, dest, &pool, &stats, &self.config)?;

        pool.join();

        Ok(())
    }
}

// ********************************************************************** //

fn queue_file_range(
    handle: &Arc<CopyHandle>,
    range: Range<u64>,
    pool: &ThreadPool,
    status_channel: &Arc<dyn StatusUpdater>,
) -> Result<u64> {
    let len = range.end - range.start;
    let bsize = handle.config.block_size;
    let blocks = (len / bsize) + (if len % bsize > 0 { 1 } else { 0 });

    for blkn in 0..blocks {
        let harc = handle.clone();
        let stat_tx = status_channel.clone();
        let bytes = cmp::min(len - (blkn * bsize), bsize);
        let off = range.start + (blkn * bsize);

        pool.execute(move || {
            let copy_result = copy_file_offset(&harc.infd, &harc.outfd, bytes, off as i64);
            let stat_result = match copy_result {
                Ok(bytes) => {
                    stat_tx.send(StatusUpdate::Copied(bytes as u64))
                }
                Err(e) => {
                    error!("Error copying: aborting.");
                    stat_tx.send(StatusUpdate::Error(XcpError::CopyError(e.to_string())))
                }
            };
            if let Err(e) = stat_result {
                let msg = format!("Failed to send status update message. This should not happen; aborting. Error: {}", e);
                error!("{}", msg);
                panic!("{}", msg);
            }
        });
    }
    Ok(len)
}

fn queue_file_blocks(
    source: &Path,
    dest: &Path,
    pool: &ThreadPool,
    status_channel: &Arc<dyn StatusUpdater>,
    config: &Arc<Config>,
) -> Result<u64> {
    let handle = CopyHandle::new(source, dest, config)?;
    let len = handle.metadata.len();

    if handle.try_reflink()? {
        info!("Reflinked, skipping rest of copy");
        return Ok(len);
    }

    // Put the open files in an Arc, which we drop once work has been
    // queued. This will keep the files open until all work has been
    // consumed, then close them. (This may be overkill; opening the
    // files in the workers would also be valid.)
    let harc = Arc::new(handle);

    let queue_whole_file = || {
        queue_file_range(&harc, 0..len, pool, status_channel)
    };

    if probably_sparse(&harc.infd)? {
        if let Some(extents) = map_extents(&harc.infd)? {
            let sparse_map = merge_extents(extents)?;
            let mut queued = 0;
            for ext in sparse_map {
                queued += queue_file_range(&harc, ext.into(), pool, status_channel)?;
            }
            Ok(queued)
        } else {
            queue_whole_file()
        }
    } else {
        queue_whole_file()
    }
}

// Dispatch worker; receives queued files and hands them to
// queue_file_blocks() which splits them onto the copy-pool.
fn dispatch_worker(file_q: cbc::Receiver<Operation>, stats: &Arc<dyn StatusUpdater>, config: Arc<Config>) -> Result<()> {
    let nworkers = config.num_workers();
    let copy_pool = Builder::new()
        .num_threads(nworkers)
        // Use bounded queue for backpressure; this limits open
        // files in-flight so we don't run out of file handles.
        // FIXME: Number is arbitrary ATM, we should be able to
        // calculate it from ulimits.
        .queue_len(128)
        .build();
    for op in file_q {
        match op {
            Operation::Copy(from, to) => {
                info!("Dispatch[{:?}]: Copy {:?} -> {:?}", thread::current().id(), from, to);
                let r = queue_file_blocks(&from, &to, &copy_pool, stats, &config);
                if let Err(e) = r {
                    stats.send(StatusUpdate::Error(XcpError::CopyError(e.to_string())))?;
                    error!("Dispatcher: Error copying {:?} -> {:?}.", from, to);
                    return Err(e)
                }
            }

            // Inline the following operations as the should be near-instant.
            Operation::Link(from, to) => {
                info!("Dispatch[{:?}]: Symlink {:?} -> {:?}", thread::current().id(), from, to);
                let r = symlink(&from, &to);
                if let Err(e) = r {
                    stats.send(StatusUpdate::Error(XcpError::CopyError(e.to_string())))?;
                    error!("Error symlinking: {:?} -> {:?}; aborting.", from, to);
                    return Err(e.into())
                }
            }

            Operation::Special(from, to) => {
                info!("Dispatch[{:?}]: Special file {:?} -> {:?}", thread::current().id(), from, to);
                copy_node(&from, &to)?;
            }
        }
    }
    info!("Queuing complete");

    copy_pool.join();
    info!("Pool complete");

    Ok(())
}
