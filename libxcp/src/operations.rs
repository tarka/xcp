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

use std::{cmp, thread};
use std::fs::{self, canonicalize, create_dir_all, read_link, File, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel as cbc;
use libfs::{
    allocate_file, copy_file_bytes, copy_permissions,
    next_sparse_segments, probably_sparse, sync, reflink, FileType, copy_timestamps,
};
use log::{debug, error, info};
use walkdir::WalkDir;

use crate::backup::{get_backup_path, needs_backup};
use crate::config::{Config, Reflink};
use crate::errors::{Result, XcpError};
use crate::feedback::{StatusUpdate, StatusUpdater};
use crate::paths::{parse_ignore, ignore_filter};

#[derive(Debug)]
pub struct CopyHandle {
    pub infd: File,
    pub outfd: File,
    pub metadata: Metadata,
    pub config: Arc<Config>,
}

impl CopyHandle {
    pub fn new(from: &Path, to: &Path, config: &Arc<Config>) -> Result<CopyHandle> {
        let infd = File::open(from)?;
        let metadata = infd.metadata()?;

        if needs_backup(to, config)? {
            let backup = get_backup_path(to)?;
            info!("Backup: Rename {:?} to {:?}", to, backup);
            fs::rename(to, backup)?;
        }

        let outfd = File::create(to)?;
        allocate_file(&outfd, metadata.len())?;

        let handle = CopyHandle {
            infd,
            outfd,
            metadata,
            config: config.clone(),
        };

        Ok(handle)
    }

    /// Copy len bytes from wherever the descriptor cursors are set.
    fn copy_bytes(&self, len: u64, updates: &Arc<dyn StatusUpdater>) -> Result<u64> {
        let mut written = 0u64;
        while written < len {
            let bytes_to_copy = cmp::min(len - written, self.config.block_size);
            let bytes = copy_file_bytes(&self.infd, &self.outfd, bytes_to_copy)? as u64;
            written += bytes;
            updates.send(StatusUpdate::Copied(bytes))?;
        }

        Ok(written)
    }

    /// Wrapper around copy_bytes that looks for sparse blocks and skips them.
    fn copy_sparse(&self, updates: &Arc<dyn StatusUpdater>) -> Result<u64> {
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
        match self.config.reflink {
            Reflink::Always | Reflink::Auto => {
                debug!("Attempting reflink from {:?}->{:?}", self.infd, self.outfd);
                let worked = reflink(&self.infd, &self.outfd)?;
                if worked {
                    debug!("Reflink {:?} succeeded", self.outfd);
                    Ok(true)
                } else if self.config.reflink == Reflink::Always {
                    Err(XcpError::ReflinkFailed(format!("{:?}->{:?}", self.infd, self.outfd)).into())
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

    pub fn copy_file(&self, updates: &Arc<dyn StatusUpdater>) -> Result<u64> {
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
        if !self.config.no_perms {
            copy_permissions(&self.infd, &self.outfd)?;
        }
        if !self.config.no_timestamps {
            copy_timestamps(&self.infd, &self.outfd)?;
        }
        if self.config.fsync {
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

#[derive(Debug)]
pub enum Operation {
    Copy(PathBuf, PathBuf),
    Link(PathBuf, PathBuf),
    Special(PathBuf, PathBuf),
}

pub fn tree_walker(
    sources: Vec<PathBuf>,
    dest: &Path,
    config: &Config,
    work_tx: cbc::Sender<Operation>,
    stats: Arc<dyn StatusUpdater>,
) -> Result<()> {
    debug!("Starting walk worker {:?}", thread::current().id());

    for source in sources {
        let sourcedir = source
            .components()
            .last()
            .ok_or(XcpError::InvalidSource("Failed to find source directory name."))?;

        let target_base = if dest.exists() && dest.is_dir() && !config.no_target_directory {
            dest.join(sourcedir)
        } else {
            dest.to_path_buf()
        };
        debug!("Target base is {:?}", target_base);

        let gitignore = parse_ignore(&source, config)?;

        for entry in WalkDir::new(&source)
            .into_iter()
            .filter_entry(|e| ignore_filter(e, &gitignore))
        {
            debug!("Got tree entry {:?}", entry);
            let epath = entry?.into_path();
            let from = if config.dereference {
                let cpath = canonicalize(&epath)?;
                debug!("Dereferencing {:?} into {:?}", epath, cpath);
                cpath
            } else {
                epath.clone()
            };
            let meta = from.symlink_metadata()?;
            let path = epath.strip_prefix(&source)?;
            let target = if !empty_path(path) {
                target_base.join(path)
            } else {
                target_base.clone()
            };

            if config.no_clobber && target.exists() {
                let msg = "Destination file exists and --no-clobber is set.";
                stats.send(StatusUpdate::Error(
                    XcpError::DestinationExists(msg, target)))?;
                return Err(XcpError::EarlyShutdown(msg).into());
            }

            let ft = FileType::from(meta.file_type());
            match ft {
                FileType::File => {
                    debug!("Send copy operation {:?} to {:?}", from, target);
                    stats.send(StatusUpdate::Size(meta.len()))?;
                    work_tx.send(Operation::Copy(from, target))?;
                }

                FileType::Symlink => {
                    let lfile = read_link(from)?;
                    debug!("Send symlink operation {:?} to {:?}", lfile, target);
                    work_tx.send(Operation::Link(lfile, target))?;
                }

                FileType::Dir => {
                    // Create dir tree immediately as we can't
                    // guarantee a worker will action the creation
                    // before a subsequent copy operation requires it.
                    debug!("Creating target directory {:?}", target);
                    if let Err(err) = create_dir_all(&target) {
                        let msg = format!("Error creating target directory: {}", err);
                        error!("{msg}");
                        return Err(XcpError::CopyError(msg).into())
                    }
                }

                FileType::Socket | FileType::Char | FileType::Fifo => {
                    debug!("Special file found: {:?} to {:?}", from, target);
                    work_tx.send(Operation::Special(from, target))?;
                }

                FileType::Block | FileType::Other => {
                    error!("Unsupported filetype found: {:?} -> {:?}", target, ft);
                    return Err(XcpError::UnknownFileType(target).into());
                }
            };
        }
    }
    debug!("Walk-worker finished: {:?}", thread::current().id());

    Ok(())
}

fn empty_path(path: &Path) -> bool {
    *path == PathBuf::new()
}
