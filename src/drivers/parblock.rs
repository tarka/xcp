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
use std::fs::{File, create_dir_all, read_link};
use std::path::{PathBuf};
use std::sync::Arc;
use std::thread;
use crossbeam_channel as cbc;
use threadpool::ThreadPool;
use log::{debug, error, info};
use walkdir::WalkDir;

use crate::errors::{Result, XcpError};
use crate::drivers::CopyDriver;
use crate::os::{allocate_file, copy_file_offset};
use crate::progress::ProgressBar;
use crate::options::{Opts, num_workers, parse_ignore, ignore_filter};
use crate::utils::{FileType, ToFileType, empty};


// ********************************************************************** //

pub struct Driver  {
}

impl CopyDriver for Driver {
    fn copy_all(&self, sources: Vec<PathBuf>, dest: PathBuf, opts: &Opts) -> Result<()> {
        copy_all(sources, dest, opts)
    }

    fn copy_single(&self, source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
        copy_single_file(source, dest, opts)
    }
}


// ********************************************************************** //

#[derive(Debug)]
struct CopyHandle {
    from: File,
    to: File,
}

// impl Drop for CopyHandle {
//     fn drop(&mut self) {
//         drop(&self.from);
//         drop(&self.to);
//         info!("Closing {:?}", self);
//     }
// }


fn queue_file_blocks(source: &PathBuf, dest: PathBuf, pool: &ThreadPool, status_channel: &cbc::Sender<Result<u64>>, bsize: u64) -> Result<()>
{
    let len = source.metadata()?.len();
    let blocks = (len / bsize) + (if len % bsize > 0 { 1 } else { 0 });

    let fhandle = CopyHandle {
        from: File::open(&source)?,
        to: File::create(&dest)?,
    };
    // Ensure target file exists up-front.
    allocate_file(&fhandle.to, len)?;

    {
        // Put the open files in an Arc, which we drop once work has
        // been queued. This will keep them open until all work has
        // been consumed, then close them. (This may be overkill;
        // opening the files in the workers would also be valid.)
        let arc = Arc::new(fhandle);

        for off in 0..blocks {
            let handle = arc.clone();
            let stat_tx = status_channel.clone();

            pool.execute(move || {
                let r = copy_file_offset(&handle.from,
                                         &handle.to,
                                         cmp::min(len - (off * bsize), bsize),
                                         (off * bsize) as i64);
                stat_tx.send(r).unwrap(); // Not much we can do if this fails.
            });
        }
    }
    Ok(())
}


pub fn copy_single_file(source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
    let nworkers = num_workers(&opts);
    let (stat_tx, stat_rx) = cbc::unbounded();
    let pool = ThreadPool::new(nworkers as usize);

    let len = source.metadata()?.len();
    let pb = ProgressBar::new(opts, len);

    queue_file_blocks(source, dest, &pool, &stat_tx, opts.block_size)?;

    // Gather the results as we go; close our end of the channel so it
    // ends when drained.
    drop(stat_tx);
    for r in stat_rx {
        pb.inc(r?);
    }
    pool.join();

    Ok(())
}

struct CopyOp {
    from: PathBuf,
    target: PathBuf
}


pub fn copy_all(sources: Vec<PathBuf>, dest: PathBuf, opts: &Opts) -> Result<()>
{
    let pb = ProgressBar::new(opts, 0);
    let mut total = 0;

    let nworkers = num_workers(&opts) as usize;
    let (stat_tx, stat_rx) = cbc::unbounded::<Result<u64>>();

    let (file_tx, file_rx) = cbc::unbounded::<CopyOp>();
    let bsize = opts.block_size;
    let stx = stat_tx.clone();
    let _dispatcher = thread::spawn(move || {
        let pool = ThreadPool::new(nworkers);
        for op in file_rx {
            info!("Queueing file {:?}", op.from);
            queue_file_blocks(&op.from, op.target, &pool, &stx, bsize)
                .unwrap(); // FIXME
        }
        pool.join();
    });

    for source in sources {

        let sourcedir = source.components().last()
            .ok_or(XcpError::InvalidSource("Failed to find source directory name."))?;

        let target_base = if dest.exists() {
            dest.join(sourcedir)
        } else {
            dest.clone()
        };
        debug!("Target base is {:?}", target_base);

        let gitignore = parse_ignore(&source, &opts)?;

        for entry in WalkDir::new(&source).into_iter()
            .filter_entry(|e| ignore_filter(e, &gitignore))
        {
            debug!("Got tree entry {:?}", entry);
            let e = entry?;
            let from = e.into_path();
            let meta = from.symlink_metadata()?;
            let path = from.strip_prefix(&source)?;
            let target = if !empty(&path) {
                target_base.join(&path)
            } else {
                target_base.clone()
            };

            if target.exists() && opts.noclobber {
                return Err(XcpError::DestinationExists(
                    "Destination file exists and --no-clobber is set.", target).into());
            }



            match meta.file_type().to_enum() {
                FileType::File => {
                    debug!("Start copy operation {:?} to {:?}", from, target);
                    file_tx.send(CopyOp {
                        from: from,
                        target: target
                    })?;
                    total += meta.len();
                    // updates.update(Ok(meta.len()))?;
                    // work_tx.send(Operation::Copy(from, target))?;
                }

                FileType::Symlink => {
                    let lfile = read_link(from)?;
                    debug!("Send symlink operation {:?} to {:?}", lfile, target);
                    // work_tx.send(Operation::Link(lfile, target))?;
                }

                FileType::Dir => {
                    debug!("Creating target directory {:?}", target);
                    create_dir_all(&target)?;
                }

                FileType::Unknown => {
                    error!("Unknown filetype found; this should never happen!");
                    return Err(XcpError::DestinationExists(
                        "Destination file exists and --no-clobber is set.", target).into());
                }
            };
        }
    }

    drop(stat_tx);
    drop(file_tx);
    pb.set_size(total);
    for r in stat_rx {
        pb.inc(r?);
    }

    Ok(())
}

