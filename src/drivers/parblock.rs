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
use std::fs::{File};
use std::path::{PathBuf};
use std::sync::Arc;

use crossbeam_channel as cbc;
use threadpool::ThreadPool;
use log::debug;

use crate::errors::Result;
use crate::drivers::CopyDriver;
use crate::os::{allocate_file, copy_file_offset};
use crate::progress::ProgressBar;
use crate::options::{Opts, num_workers};


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

impl Drop for CopyHandle {
    fn drop(&mut self) {
        debug!("Closing {:?}", self);
    }
}

pub fn copy_single_file(source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
    let nworkers = num_workers(&opts);
    let (stat_tx, stat_rx) = cbc::unbounded();
    let pool = ThreadPool::new(nworkers as usize);


    let bsize = opts.block_size;
    let len = source.metadata()?.len();
    let blocks = (len / bsize) + (if len % bsize > 0 { 1 } else { 0 });

    let pb = ProgressBar::new(opts, len);

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
            let stat = stat_tx.clone();

            pool.execute(move || {
                let r = copy_file_offset(&handle.from,
                                         &handle.to,
                                         cmp::min(len - (off * bsize), bsize),
                                         (off * bsize) as i64);
                stat.send(r).unwrap(); // Not much we can do if this fails.
            });
        }
    }

    // Gather the results as we go; clouse our end of the channel so
    // it ends when drained.
    drop(stat_tx);
    for r in stat_rx {
        pb.inc(r?);
    }
    pool.join();

    Ok(())
}


pub fn copy_all(_sources: Vec<PathBuf>, _dest: PathBuf, _opts: &Opts) -> Result<()> {
    panic!()
}

