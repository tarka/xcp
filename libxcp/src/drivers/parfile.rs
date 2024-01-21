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

use crossbeam_channel as cbc;
use log::{debug, error, info};
use libfs::copy_node;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use crate::config::Config;
use crate::drivers::CopyDriver;
use crate::errors::{Result, XcpError};
use crate::operations::{CopyHandle, StatusUpdate, StatusUpdater, Operation, tree_walker};

// ********************************************************************** //

pub struct Driver {
    config: Arc<Config>,
}

impl CopyDriver for Driver {
    fn new(config: Arc<Config>) -> Result<Self> {
        Ok(Self {
            config,
        })
    }

    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, stats: Arc<dyn StatusUpdater>) -> Result<()> {
        let (work_tx, work_rx) = cbc::unbounded();

        // Thread which walks the file tree and sends jobs to the
        // workers. The worker tx channel is moved to the walker so it is
        // closed, which will cause the workers to shutdown on completion.
        let _walk_worker = {
            let sc = stats.clone();
            let d = dest.to_path_buf();
            let o = self.config.clone();
            thread::spawn(move || tree_walker(sources, &d, &o, work_tx, sc))
        };

        // Worker threads. Will consume work and then shutdown once the
        // queue is closed by the walker.
        for _ in 0..self.config.num_workers() {
            let _copy_worker = {
                let wrx = work_rx.clone();
                let sc = stats.clone();
                let o = self.config.clone();
                thread::spawn(move || copy_worker(wrx, &o, sc))
            };
        }

        // FIXME: Ideally we should join the dispatch and walker
        // threads to ensure we pickup any errors not on the
        // queue. However this would block until all work was
        // dispatched, blocking progress bar updates.

        Ok(())
    }

    fn copy_single(&self, source: &Path, dest: &Path, stats: Arc<dyn StatusUpdater>) -> Result<()> {
        let handle = CopyHandle::new(source, dest, &self.config)?;
        handle.copy_file(&stats)?;
        Ok(())
    }
}

// ********************************************************************** //

fn copy_worker(
    work: cbc::Receiver<Operation>,
    config: &Arc<Config>,
    updates: Arc<dyn StatusUpdater>,
) -> Result<()> {
    debug!("Starting copy worker {:?}", thread::current().id());
    for op in work {
        debug!("Received operation {:?}", op);

        match op {
            Operation::Copy(from, to) => {
                info!("Worker[{:?}]: Copy {:?} -> {:?}", thread::current().id(), from, to);
                // copy_file() sends back its own updates, but we should
                // send back any errors as they may have occurred
                // before the copy started..
                let r = CopyHandle::new(&from, &to, config)
                    .and_then(|hdl| hdl.copy_file(&updates));
                if let Err(e) = r {
                    updates.send(StatusUpdate::Error(XcpError::CopyError(e.to_string())))?;
                    error!("Error copying: {:?} -> {:?}; aborting.", from, to);
                    return Err(e)
                }
            }

            Operation::Link(from, to) => {
                info!("Worker[{:?}]: Symlink {:?} -> {:?}", thread::current().id(), from, to);
                let _r = symlink(&from, &to);
            }

            Operation::Special(from, to) => {
                info!("Worker[{:?}]: Special file {:?} -> {:?}", thread::current().id(), from, to);
                copy_node(&from, &to)?;
            }

        }
    }
    debug!("Copy worker {:?} shutting down", thread::current().id());
    Ok(())
}
