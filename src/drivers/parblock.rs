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
use std::cmp;
use std::fs::{File, OpenOptions, create_dir_all, read_link};
use std::io::ErrorKind as IOKind;
use std::os::unix::fs::symlink;
use std::path::{PathBuf};
use std::sync::Arc;
use std::thread;
use walkdir::{WalkDir};

use crate::errors::{io_err, Result, XcpError};
use crate::drivers::{CopyDriver};
use crate::os::{allocate_file, copy_file_offset};
use crate::progress::{
    iprogress_bar, BatchUpdater, NopUpdater, ProgressBar, ProgressUpdater, StatusUpdate, Updater,
    BATCH_DEFAULT,
};
use crate::utils::{FileType, ToFileType, empty};
use crate::options::{Opts, num_workers, parse_ignore, ignore_filter};


// ********************************************************************** //

pub struct Driver  {
}

impl CopyDriver for Driver {
    fn copy_all(&self, sources: Vec<PathBuf>, dest: PathBuf, opts: &Opts) -> Result<()> {
        copy_all(sources, dest, opts)
    }

    fn copy_single(&self, source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
        debug!("CALLING SINGLE");
        copy_single_file(source, dest, opts)
    }
}


// ********************************************************************** //

#[derive(Debug)]
struct CopyHandle {
    from: File,
    to: File,
}

#[derive(Debug)]
struct CopyOp {
    handle: Arc<CopyHandle>,
    start: u64,
    bytes: u64,
}


fn copy_worker(work: cbc::Receiver<CopyOp>) -> Result<()> {
    info!("Starting copy worker {:?}", thread::current().id());
    for op in work {
        info!("Worker {:?}: Copy {:?}", thread::current().id(), op);

        let r = copy_file_offset(&op.handle.from, &op.handle.to, op.bytes, op.start as i64);
        if !r.is_ok() {
            error!("Error copying: {:?}", r);
            r?;
        }
    }
    info!("Copy worker {:?} shutting down", thread::current().id());
    Ok(())
}


pub fn copy_single_file(source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
    let nworkers = num_workers(&opts);
    let (work_tx, work_rx) = cbc::unbounded();

    info!("Spawning {:?} workers", nworkers);
    let mut thandles = Vec::with_capacity(nworkers as usize);
    for _ in 0..nworkers {
        let worker = {
            let wrx = work_rx.clone();
            thread::spawn(|| copy_worker(wrx))
        };
        thandles.push(worker);
    }

    let bsize = 1024*1024;

    let len = source.metadata()?.len();
    let blocks = (len / bsize) + (if len % bsize > 0 { 1 } else { 0 });

    {
        let fhandle = CopyHandle {
            from: File::open(&source)?,
            to: File::create(&dest)?,
        };
        // Ensure target file exists up-front.
        allocate_file(&fhandle.to, len)?;

        // Put the open files in an Arc, which we drop once work has
        // been queued. This will keep them open until all work has
        // been consumed, then close them. (This may be overkill;
        // opening the files in the workers would also be valid.)
        let arc = Arc::new(fhandle);
        for off in 0..blocks {
            let op = CopyOp {
                handle: arc.clone(),
                start: off * bsize,
                bytes: cmp::min(len - (off * bsize), bsize)
            };
            work_tx.send(op)?;
        }
    }

    // Close the sender end of the work queue; this will trigger the
    // workers to shut down when the queue is drained.
    drop(work_tx);

    for h in thandles {
        let _ = h.join();
    }

    Ok(())
}


pub fn copy_all(_sources: Vec<PathBuf>, _dest: PathBuf, _opts: &Opts) -> Result<()> {
    panic!()
}

