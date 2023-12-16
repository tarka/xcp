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
use std::fs::{create_dir_all, read_link};
use std::ops::Range;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use cfg_if::cfg_if;
use crossbeam_channel as cbc;
use libfs::{FileType, copy_node};
use log::{debug, error, info};
use blocking_threadpool::{Builder, ThreadPool};
use walkdir::WalkDir;

use crate::drivers::CopyDriver;
use crate::errors::{Result, XcpError};
use crate::operations::CopyHandle;
use crate::options::{ignore_filter, num_workers, parse_ignore, Opts};
use libfs::{copy_file_offset, map_extents, merge_extents, probably_sparse};
use crate::progress::{ProgressBar, StatusUpdate};
use crate::utils::empty;

// ********************************************************************** //

const fn supported_platform() -> bool {
    cfg_if! {
        if #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os="dragonfly"))] {
            true
        } else {
            false
        }
    }
}


pub struct Driver;

impl Driver {
    pub fn new(_opts: &Opts) -> Result<Self> {
        if !supported_platform() {
            let msg = "The parblock driver is not currently supported on this OS.";
            error!("{}", msg);
            return Err(XcpError::UnsupportedOS(msg).into());
        }

        Ok(Self {})
    }
}

impl CopyDriver for Driver {

    fn copy_all(&self, sources: Vec<PathBuf>, dest: &Path, opts: Arc<Opts>) -> Result<()> {
        copy_all(sources, dest, opts)
    }

    fn copy_single(&self, source: &Path, dest: &Path, opts: Arc<Opts>) -> Result<()> {
        copy_single_file(source, dest, opts)
    }
}

// ********************************************************************** //

// FIXME: We should probably move this to the progress-bar module and
// abstract away more of the channel setup to be no-ops when
// --no-progress is specified.
static BYTE_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Sender {
    noop: bool,
    chan: cbc::Sender<StatusUpdate>,
}
impl Sender {
    fn new(chan: cbc::Sender<StatusUpdate>, opts: &Opts) -> Sender {
        Sender {
            noop: opts.no_progress,
            chan,
        }
    }

    fn send(&self, update: StatusUpdate, bytes: u64, bsize: u64) -> Result<()> {
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

fn queue_file_range(
    handle: &Arc<CopyHandle>,
    range: Range<u64>,
    pool: &ThreadPool,
    status_channel: &Sender,
) -> Result<u64> {
    let len = range.end - range.start;
    let bsize = handle.opts.block_size;
    let blocks = (len / bsize) + (if len % bsize > 0 { 1 } else { 0 });

    for blkn in 0..blocks {
        let harc = handle.clone();
        let stat_tx = status_channel.clone();
        let bytes = cmp::min(len - (blkn * bsize), bsize);
        let off = range.start + (blkn * bsize);

        pool.execute(move || {
            // FIXME: Move into CopyHandle once settled.
            let r = copy_file_offset(&harc.infd, &harc.outfd, bytes, off as i64).unwrap();

            stat_tx.send(StatusUpdate::Copied(r as u64), bytes, bsize).unwrap();
        });
    }
    Ok(len)
}


fn queue_file_blocks(
    source: &Path,
    dest: &Path,
    pool: &ThreadPool,
    status_channel: &Sender,
    opts: Arc<Opts>,
) -> Result<u64> {
    let handle = CopyHandle::new(source, dest, opts)?;
    let len = handle.metadata.len();

    // Put the open files in an Arc, which we drop once work has
    // been queued. This will keep them open until all work has
    // been consumed, then close them. (This may be overkill;
    // opening the files in the workers would also be valid.)
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

fn copy_single_file(source: &Path, dest: &Path, opts: Arc<Opts>) -> Result<()> {
    let nworkers = num_workers(&opts);
    let pool = ThreadPool::new(nworkers as usize);

    let len = source.metadata()?.len();
    let pb = ProgressBar::new(&opts, len)?;

    let (stat_tx, stat_rx) = cbc::unbounded();
    let sender = Sender::new(stat_tx, &opts);
    queue_file_blocks(source, dest, &pool, &sender, opts)?;
    drop(sender);

    // Gather the results as we go; close our end of the channel so it
    // ends when drained.
    for r in stat_rx {
        pb.inc(r.value());
    }
    pool.join();
    pb.end();

    Ok(())
}

struct CopyOp {
    from: PathBuf,
    target: PathBuf,
}

fn copy_all(sources: Vec<PathBuf>, dest: &Path, opts: Arc<Opts>) -> Result<()> {
    let pb = ProgressBar::new(&opts, 0)?;
    let mut total = 0;

    let nworkers = num_workers(&opts) as usize;
    let (stat_tx, stat_rx) = cbc::unbounded::<StatusUpdate>();

    let (file_tx, file_rx) = cbc::unbounded::<CopyOp>();
    let sender = Sender::new(stat_tx, &opts);
    let q_opts = opts.clone();
    let _dispatcher = thread::spawn(move || {
        let pool = Builder::new()
            .num_threads(nworkers)
            // Use bounded queue for backpressure; this limits open
            // files in-flight so we don't run out of file handles.
            // FIXME: Number is arbitrary ATM, we should be able to
            // calculate it from ulimits.
            .queue_len(128)
            .build();
        for op in file_rx {
            queue_file_blocks(&op.from, &op.target, &pool, &sender, q_opts.clone()).unwrap();
            // FIXME
        }
        info!("Queuing complete");

        pool.join();
        info!("Pool complete");
    });

    for source in sources {
        let sourcedir = source.components().last().ok_or(XcpError::InvalidSource(
            "Failed to find source directory name.",
        ))?;

        let target_base = if dest.exists() {
            dest.join(sourcedir)
        } else {
            dest.to_path_buf()
        };
        debug!("Target base is {:?}", target_base);

        let gitignore = parse_ignore(&source, &opts.clone())?;

        for entry in WalkDir::new(&source)
            .into_iter()
            .filter_entry(|e| ignore_filter(e, &gitignore))
        {
            debug!("Got tree entry {:?}", entry);
            let e = entry?;
            let from = e.into_path();
            let meta = from.symlink_metadata()?;
            let path = from.strip_prefix(&source)?;
            let target = if !empty(path) {
                target_base.join(path)
            } else {
                target_base.clone()
            };

            if opts.no_clobber && target.exists() {
                return Err(XcpError::DestinationExists(
                    "Destination file exists and --no-clobber is set.",
                    target,
                )
                .into());
            }

            match FileType::from(meta.file_type()) {
                FileType::File => {
                    debug!("Start copy operation {:?} to {:?}", from, target);
                    file_tx.send(CopyOp {
                        from,
                        target,
                    })?;
                    total += meta.len();
                }

                FileType::Symlink => {
                    let lfile = read_link(from)?;
                    debug!("Creating symlink from {:?} to {:?}", lfile, target);
                    let _r = symlink(&lfile, &target);
                }

                FileType::Dir => {
                    debug!("Creating target directory {:?}", target);
                    create_dir_all(&target)?;
                }

                FileType::Socket | FileType::Char | FileType::Fifo => {
                    debug!("Copy special file {:?} to {:?}", from, target);
                    copy_node(&from, &target)?;
                }

                FileType::Other => {
                    error!("Unknown filetype found; this should never happen!");
                    return Err(XcpError::UnknownFileType(target).into());
                }
            };
        }
    }

    drop(file_tx);
    pb.set_size(total);
    for up in stat_rx {
        match up {
            StatusUpdate::Copied(v) => pb.inc(v),
            StatusUpdate::Size(v) => pb.inc_size(v),
        }
    }
    pb.end();

    Ok(())
}
