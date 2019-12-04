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

use std::path::PathBuf;
use std::result;

use glob::{glob, Paths};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use num_cpus;
use structopt::StructOpt;
use unbytify::unbytify;
use walkdir::{DirEntry};

use crate::errors::Result;
use crate::drivers::Drivers;


#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "xcp",
    about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.",
    setting = structopt::clap::AppSettings::ColoredHelp
)]
pub struct Opts {
    /// Explain what is being done. Can be specified multiple times to
    /// increase logging.
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u64,

    /// Copy directories recursively
    #[structopt(short = "r", long = "recursive")]
    pub recursive: bool,

    /// Number of parallel workers for recursive copies. Default is 1;
    /// if the value is negative or 0 it uses the number of logical CPUs.
    #[structopt(short = "w", long = "workers",  default_value = "4")]
    pub workers: i64,

    /// Block size for operations. Accepts standard size modifiers
    /// like "M" and "GB". Actual usage internally depends on the
    /// driver.
    #[structopt(long = "block-size",  default_value = "1MB", parse(try_from_str = unbytify))]
    pub block_size: u64,

    /// Do not overwrite an existing file
    #[structopt(short = "n", long = "no-clobber")]
    pub noclobber: bool,

    /// Use .gitignore if present. NOTE: This is fairly basic at the
    /// moment, and only honours a .gitignore in the directory root
    /// for directory copies; global or sub-directory ignores are
    /// skipped.
    #[structopt(long = "gitignore")]
    pub gitignore: bool,

    /// Glob (expand) filename patterns natively (note; the shell may still do its own expansion first)
    #[structopt(short = "g", long = "glob")]
    pub glob: bool,

    /// Disable progress bar.
    #[structopt(long = "no-progress")]
    pub noprogress: bool,

    /// Do not copy the file permissions.
    #[structopt(long = "no-perms")]
    pub no_perms: bool,

    /// Specify the driver. Currently there are 2; the default
    /// "parfile", which parallelises copies across workers at the
    /// file level, and an experimental "parblock" driver, which
    /// parellelises at the block level. See also '--block-size'.
    #[structopt(long = "driver")]
    pub driver: Option<Drivers>,

    /// Analagous to cp's no-target-directory. Expected behavior is that when
    /// copying a directory to another directory, instead of creating a sub-folder
    /// in target, overwrite target.
    #[structopt(short = "T", long = "no-target-directory" )]
    pub no_target_directory: bool,

    #[structopt(required = true, min_values = 2 )]
    pub paths: Vec<String>,

}


// StructOpt handles optional flags with optional values as nested Options.
pub fn num_workers(opts: &Opts) -> u64 {
    if opts.workers <= 0 {
        num_cpus::get() as u64
    } else {
        opts.workers as u64
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
pub fn expand_globs(patterns: &[String]) -> Result<Vec<PathBuf>> {
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

pub fn to_pathbufs(paths: &[String]) -> Vec<PathBuf> {
    paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<PathBuf>>()
}

pub fn expand_sources(source_list: &[String], opts: &Opts) -> Result<Vec<PathBuf>> {
    if opts.glob {
        expand_globs(&source_list)
    } else {
        Ok(to_pathbufs(source_list))
    }
}

pub fn parse_ignore(source: &PathBuf, opts: &Opts) -> Result<Option<Gitignore>> {
    let gitignore = if opts.gitignore {
        let mut builder = GitignoreBuilder::new(&source);
        builder.add(&source.join(".gitignore"));
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
            let m = gi.matched(&path, path.is_dir());
            !m.is_ignore()
        }
    }
}
