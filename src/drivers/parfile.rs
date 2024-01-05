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
use libfs::{FileType, copy_node};
use std::fs::{create_dir_all, read_link};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use walkdir::WalkDir;

use crate::drivers::CopyDriver;
use crate::errors::{Result, XcpError};
use crate::operations::{CopyHandle, StatusUpdate, StatSender};
use crate::options::{ignore_filter, parse_ignore, Opts};
use crate::utils::empty;

// ********************************************************************** //

pub struct Driver {
    opts: Arc<Opts>,
}

impl CopyDriver for Driver {
    fn new(opts: Arc<Opts>) -> Result<Self> {
        Ok(Self {
            opts,
        })
    }

    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, stats: StatSender) -> Result<()> {
        let (work_tx, work_rx) = cbc::unbounded();

        // Thread which walks the file tree and sends jobs to the
        // workers. The worker tx channel is moved to the walker so it is
        // closed, which will cause the workers to shutdown on completion.
        let _walk_worker = {
            let sc = stats.clone();
            let d = dest.to_path_buf();
            let o = self.opts.clone();
            thread::spawn(move || tree_walker(sources, &d, &o, work_tx, sc))
        };

        // Worker threads. Will consume work and then shutdown once the
        // queue is closed by the walker.
        for _ in 0..self.opts.num_workers() {
            let _copy_worker = {
                let wrx = work_rx.clone();
                let sc = stats.clone();
                let o = self.opts.clone();
                thread::spawn(move || copy_worker(wrx, &o, sc))
            };
        }

        // FIXME: We should probably join the threads and consume any errors.

        Ok(())
    }

    fn copy_single(&self, source: &Path, dest: &Path, stats: StatSender) -> Result<()> {
        let handle = CopyHandle::new(source, dest, &self.opts)?;
        handle.copy_file(&stats)?;
        Ok(())
    }
}

// ********************************************************************** //

#[derive(Debug)]
enum Operation {
    Copy(PathBuf, PathBuf),
    Link(PathBuf, PathBuf),
    Special(PathBuf, PathBuf),
}

fn copy_worker(
    work: cbc::Receiver<Operation>,
    opts: &Arc<Opts>,
    updates: StatSender,
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
                let r = CopyHandle::new(&from, &to, opts)
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

fn copy_source(
    source: &Path,
    dest: &Path,
    opts: &Opts,
    work_tx: &cbc::Sender<Operation>,
    updates: &StatSender,
) -> Result<()> {
    let sourcedir = source
        .components()
        .last()
        .ok_or(XcpError::InvalidSource(
            "Failed to find source directory name.",
        ))?;

    let target_base = if dest.exists() && !opts.no_target_directory {
        dest.join(sourcedir)
    } else {
        dest.to_path_buf()
    };
    debug!("Target base is {:?}", target_base);

    let gitignore = parse_ignore(source, opts)?;

    for entry in WalkDir::new(source)
        .into_iter()
        .filter_entry(|e| ignore_filter(e, &gitignore))
    {
        debug!("Got tree entry {:?}", entry);
        let e = entry?;
        let from = e.into_path();
        let meta = from.symlink_metadata()?;
        let path = from.strip_prefix(source)?;
        let target = if !empty(path) {
            target_base.join(path)
        } else {
            target_base.clone()
        };

        if opts.no_clobber && target.exists() {
            updates.send(StatusUpdate::Error(
                XcpError::DestinationExists("Destination file exists and --no-clobber is set.", target)))?;
            return Err(XcpError::EarlyShutdown("Path exists and --no-clobber set.").into());
        }

        match FileType::from(meta.file_type()) {
            FileType::File => {
                debug!("Send copy operation {:?} to {:?}", from, target);
                updates.send(StatusUpdate::Size(meta.len()))?;
                work_tx.send(Operation::Copy(from, target))?;
            }

            FileType::Symlink => {
                let lfile = read_link(from)?;
                debug!("Send symlink operation {:?} to {:?}", lfile, target);
                work_tx.send(Operation::Link(lfile, target))?;
            }

            FileType::Dir => {
                debug!("Creating target directory {:?}", target);
                create_dir_all(&target)?;
            }

            FileType::Socket | FileType::Char | FileType::Fifo => {
                debug!("Special file found: {:?} to {:?}", from, target);
                work_tx.send(Operation::Special(from, target))?;
            }

            FileType::Other => {
                error!("Unknown filetype found; this should never happen!");
                return Err(XcpError::UnknownFileType(target).into());
            }
        };
    }

    Ok(())
}

fn tree_walker(
    sources: Vec<PathBuf>,
    dest: &Path,
    opts: &Opts,
    work_tx: cbc::Sender<Operation>,
    updates: StatSender,
) -> Result<()> {
    debug!("Starting walk worker {:?}", thread::current().id());

    for source in sources {
        copy_source(&source, dest, opts, &work_tx, &updates)?;
    }
    debug!("Walk-worker finished: {:?}", thread::current().id());
    Ok(())
}
