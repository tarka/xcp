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
use std::fs::{create_dir_all, read_link};
use std::io::ErrorKind as IOKind;
use std::os::unix::fs::symlink;
use std::path::{PathBuf};
use std::thread;
use walkdir::{WalkDir};

use crate::errors::{io_err, Result, XcpError};
use crate::drivers::{CopyDriver};
use crate::operations::{copy_file};
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


pub fn copy_all(sources: Vec<PathBuf>, dest: PathBuf, opts: &Opts) -> Result<()> {
    panic!()
}


pub fn copy_single_file(source: &PathBuf, dest: PathBuf, opts: &Opts) -> Result<()> {
    panic!()
}
