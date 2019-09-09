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
use std::fs::{File, OpenOptions, create_dir_all, read_link};
use std::io::ErrorKind as IOKind;
use std::os::unix::fs::symlink;
use std::path::{PathBuf};
use std::sync::Arc;
use std::thread;

use crossbeam_channel as cbc;
use threadpool::ThreadPool;
use log::{debug, error, info};
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
    let pool = ThreadPool::new(nworkers as usize);

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
            let handle = arc.clone();
            pool.execute(move || {
                let _r = copy_file_offset(&handle.from,
                                          &handle.to,
                                          cmp::min(len - (off * bsize), bsize),
                                          (off * bsize) as i64);
            });
        }
    }

    pool.join();

    Ok(())
}


pub fn copy_all(_sources: Vec<PathBuf>, _dest: PathBuf, _opts: &Opts) -> Result<()> {
    panic!()
}

