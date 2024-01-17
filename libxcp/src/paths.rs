/*
 * Copyright © 2024, Steve Smith <tarkasteve@gmail.com>
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

use std::path::{Path, PathBuf};
use std::result;

use glob::{glob, Paths};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::info;
use walkdir::DirEntry;

use crate::errors::Result;
use crate::options::Opts;

/// Expand a list of file-paths or glob-patterns into a list of concrete paths.
pub fn expand_globs(patterns: &[String]) -> Result<Vec<PathBuf>> {
    // Note: This is probably iterator overkill, but it took me a
    // whole day to work this out and I'm not prepared to give it up
    // yet.
    //
    // FIXME: This currently eats non-existent files that are not
    // globs. Should we convert empty glob results into errors?
    let mut globs = patterns
        .iter()
        .map(|s| glob(s.as_str())) // -> Vec<Result<Paths>>
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

pub fn expand_sources(source_list: &[String], opts: &Opts) -> Result<Vec<PathBuf>> {
    if opts.glob {
        expand_globs(source_list)
    } else {
        let pb = source_list.iter()
            .map(PathBuf::from)
            .collect::<Vec<PathBuf>>();
        Ok(pb)
    }
}

pub fn parse_ignore(source: &Path, opts: &Opts) -> Result<Option<Gitignore>> {
    let gitignore = if opts.gitignore {
        let gifile = source.join(".gitignore");
        info!("Using .gitignore file {:?}", gifile);
        let mut builder = GitignoreBuilder::new(source);
        builder.add(&gifile);
        let ignore = builder.build()?;
        Some(ignore)
    } else {
        None
    };
    Ok(gitignore)
}

pub fn ignore_filter(entry: &DirEntry, ignore: &Option<Gitignore>) -> bool {
    match ignore {
        None => true,
        Some(gi) => {
            let path = entry.path();
            let m = gi.matched(path, path.is_dir());
            !m.is_ignore()
        }
    }
}
