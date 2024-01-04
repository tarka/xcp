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
use crate::operations::{CopyHandle, StatusUpdate};
use crate::options::{ignore_filter, num_workers, parse_ignore, Opts};
use crate::progress::{
    BatchUpdater, NopUpdater, ProgressBar, ProgressUpdater, Updater,
    BATCH_DEFAULT,
};
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

    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path) -> Result<()> {
        copy_all(sources, dest, &self.opts)
    }

    fn copy_single(&self, source: &Path, dest: &Path) -> Result<()> {
        copy_single_file(source, dest, &self.opts)
    }
}

// ********************************************************************** //

#[derive(Debug)]
enum Operation {
    Copy(PathBuf, PathBuf),
    Link(PathBuf, PathBuf),
    Special(PathBuf, PathBuf),
    End,
}

fn copy_worker(
    work: cbc::Receiver<Operation>,
    opts: &Arc<Opts>,
    mut updates: BatchUpdater,
) -> Result<()> {
    debug!("Starting copy worker {:?}", thread::current().id());
    for op in work {
        debug!("Received operation {:?}", op);

        match op {
            Operation::Copy(from, to) => {
                info!("Worker: Copy {:?} -> {:?}", from, to);
                // copy_file() sends back its own updates, but we should
                // send back any errors as they may have occurred
                // before the copy started..
                let r = CopyHandle::new(&from, &to, opts)
                    .and_then(|hdl| hdl.copy_file(&mut updates));
                if r.is_err() {
                    updates.update(r)?;
                    error!("Error copying: {:?} -> {:?}; aborting.", from, to);
                    // TODO: Option to continue on error?
                    break;
                }
            }

            Operation::Link(from, to) => {
                info!("Worker: Symlink {:?} -> {:?}", from, to);
                let _r = symlink(&from, &to);
            }

            Operation::Special(from, to) => {
                info!("Worker: Special file {:?} -> {:?}", from, to);
                copy_node(&from, &to)?;
            }

            Operation::End => {
                info!("Worker received shutdown command.");
                break;
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
    updates: &mut BatchUpdater,
) -> Result<()> {
    let sourcedir = source.components().last().ok_or(XcpError::InvalidSource(
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
            work_tx.send(Operation::End)?;
            updates.update(Err(XcpError::DestinationExists(
                "Destination file exists and --no-clobber is set.",
                target,
            )
            .into()))?;
            return Err(XcpError::EarlyShutdown("Path exists and --no-clobber set.").into());
        }

        match FileType::from(meta.file_type()) {
            FileType::File => {
                debug!("Send copy operation {:?} to {:?}", from, target);
                updates.update(Ok(meta.len()))?;
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
    mut updates: BatchUpdater,
) -> Result<()> {
    debug!("Starting walk worker {:?}", thread::current().id());

    for source in sources {
        copy_source(&source, dest, opts, &work_tx, &mut updates)?;
    }
    work_tx.send(Operation::End)?;
    debug!("Walk-worker finished: {:?}", thread::current().id());
    Ok(())
}

pub fn copy_all(sources: Vec<PathBuf>, dest: &Path, opts: &Arc<Opts>) -> Result<()> {
    let (work_tx, work_rx) = cbc::unbounded();
    let (stat_tx, stat_rx) = cbc::unbounded();

    let (pb, batch_size) = if opts.no_progress {
        (ProgressBar::Nop, usize::max_value() as u64)
    } else {
        (ProgressBar::new(&opts, 0)?, BATCH_DEFAULT)
    };

    // Use scoped threads here so we can pass down e.g. Opts without
    // repeated cloning.
    thread::scope(|s| {
        for _ in 0..num_workers(&opts) {
            let _copy_worker = {
                let copy_stat = BatchUpdater {
                    sender: Box::new(stat_tx.clone()),
                    stat: StatusUpdate::Copied(0),
                    batch_size,
                };
                let wrx = work_rx.clone();
                s.spawn(move || copy_worker(wrx, opts, copy_stat))
            };
        }
        let _walk_worker = {
            let size_stat = BatchUpdater {
                sender: Box::new(stat_tx),
                stat: StatusUpdate::Size(0),
                batch_size,
            };
            s.spawn(|| tree_walker(sources, dest, &opts, work_tx, size_stat))
        };

        for stat in stat_rx {
            match stat? {
                StatusUpdate::Size(s) => {
                    pb.inc_size(s);
                }
                StatusUpdate::Copied(s) => {
                    pb.inc(s);
                }
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    // FIXME: We should probably join the threads and consume any errors.

    pb.end();
    debug!("Copy complete");

    Ok(())
}

fn copy_single_file(source: &Path, dest: &Path, opts: &Arc<Opts>) -> Result<()> {
    let mut copy_stat = if opts.no_progress {
        BatchUpdater {
            sender: Box::new(NopUpdater {}),
            stat: StatusUpdate::Copied(0),
            batch_size: usize::max_value() as u64,
        }
    } else {
        let size = source.metadata()?.len();
        BatchUpdater {
            sender: Box::new(ProgressUpdater {
                pb: ProgressBar::new(&opts, size)?,
                written: 0,
            }),
            stat: StatusUpdate::Copied(0),
            batch_size: BATCH_DEFAULT,
        }
    };

    let handle = CopyHandle::new(source, dest, opts)?;
    handle.copy_file(&mut copy_stat)?;

    Ok(())
}
