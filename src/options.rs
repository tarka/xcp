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
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "xcp",
    about = "Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
pub struct Opts {
    /// Explain what is being done. Can be specified multiple times to
    /// increase logging.
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u64,

    /// Copy directories recursively
    #[structopt(short = "r", long = "recursive")]
    pub recursive: bool,

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

    #[structopt(raw(required = "true", min_values = "1"))]
    pub source_list: Vec<String>,

    #[structopt(parse(from_os_str))]
    pub dest: PathBuf,
}
