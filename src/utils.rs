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

use std::fs;
use std::path::PathBuf;
use std::result;

use glob::{glob, Paths};

use crate::options::Opts;
use crate::errors::Result;

pub enum FileType {
    File,
    Dir,
    Symlink,
    Unknown,
}

pub trait ToFileType {
    fn to_enum(&self) -> FileType;
}

fn to_enum(ft: &fs::FileType) -> FileType {
    if ft.is_dir() {
        FileType::Dir
    } else if ft.is_file() {
        FileType::File
    } else if ft.is_symlink() {
        FileType::Symlink
    } else {
        FileType::Unknown
    }
}

impl ToFileType for fs::FileType {
    fn to_enum(&self) -> FileType {
        to_enum(self)
    }
}

// Expand a list of file-paths or glob-patterns into a list of concrete paths.
//
// Note: This is probably iterator overkill, but it took me a whole
// day to work this out and I'm not prepared to give it up yet.
//
// FIXME: This currently eats non-existent files that are not
// globs. Should we convert empty glob results into errors?
//
pub fn expand_globs(patterns: &Vec<String>) -> Result<Vec<PathBuf>> {
    let mut globs = patterns
        .iter()
        .map(|s| glob(&*s.as_str())) // -> Vec<Result<Paths>>
        .collect::<result::Result<Vec<Paths>, _>>()?; // -> Result<Vec<Paths>>
    let path_vecs = globs
        .iter_mut()
        // Force resolve each glob Paths iterator into a vector of the results...
        .map::<result::Result<Vec<PathBuf>, _>, _>(|p| p.collect())
        // And lift all the results up to the top.
        .collect::<result::Result<Vec<Vec<PathBuf>>, _>>()?;
    // And finally flatten the nested paths into a single collection of the results
    let paths = path_vecs
        .iter()
        .flat_map(|p| p.to_owned())
        .collect::<Vec<PathBuf>>();

    Ok(paths)
}

pub fn to_pathbufs(opts: &Opts) -> Result<Vec<PathBuf>> {
    if opts.glob {
        expand_globs(&opts.source_list)
    } else {
        let vec = opts.source_list
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<PathBuf>>();
        Ok(vec)
    }
}

