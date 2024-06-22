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

use clap::{ArgAction, Parser};

use libxcp::config::{Config, Reflink, Backup};
use log::LevelFilter;
use unbytify::unbytify;

use libxcp::drivers::Drivers;
use libxcp::errors::Result;

#[derive(Clone, Debug, Parser)]
#[command(
    name = "xcp",
    about = "A (partial) clone of the Unix `cp` command with progress and pluggable drivers.",
    version,
)]
pub struct Opts {
    /// Verbosity.
    ///
    /// Can be specified multiple times to increase logging.
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    /// Copy directories recursively
    #[arg(short, long)]
    pub recursive: bool,

    /// Dereference symlinks in source
    ///
    /// Follow symlinks, possibly recursively, when copying source
    /// files.
    #[arg(short = 'L', long)]
    pub dereference: bool,

    /// Number of parallel workers.
    ///
    /// Default is 4; if the value is negative or 0 it uses the number
    /// of logical CPUs.
    #[arg(short, long, default_value = "4")]
    pub workers: usize,

    /// Block size for operations.
    ///
    /// Accepts standard size modifiers like "M" and "GB". Actual
    /// usage internally depends on the driver.
    #[arg(long,  default_value = "1MB", value_parser=unbytify)]
    pub block_size: u64,

    /// Do not overwrite an existing file
    #[arg(short, long)]
    pub no_clobber: bool,

    /// Use .gitignore if present.
    ///
    /// NOTE: This is fairly basic at the moment, and only honours a
    /// .gitignore in the directory root for directory copies; global
    /// or sub-directory ignores are skipped.
    #[arg(long)]
    pub gitignore: bool,

    /// Expand file patterns.
    ///
    /// Glob (expand) filename patterns natively (note; the shell may still do its own expansion first)
    #[arg(short, long)]
    pub glob: bool,

    /// Disable progress bar.
    #[arg(long)]
    pub no_progress: bool,

    /// Do not copy the file permissions.
    #[arg(long)]
    pub no_perms: bool,

    /// Do not copy the file timestamps.
    #[arg(long)]
    pub no_timestamps: bool,

    /// Driver to use, defaults to 'file-parallel'.
    ///
    /// Currently there are 2; the default "parfile", which
    /// parallelises copies across workers at the file level, and an
    /// experimental "parblock" driver, which parellelises at the
    /// block level. See also '--block-size'.
    #[arg(long, default_value = "parfile")]
    pub driver: Drivers,

    /// Target should not be a directory.
    ///
    /// Analogous to cp's no-target-directory. Expected behavior is that when
    /// copying a directory to another directory, instead of creating a sub-folder
    /// in target, overwrite target.
    #[arg(short = 'T', long)]
    pub no_target_directory: bool,

    /// Sync each file to disk after writing.
    #[arg(long)]
    pub fsync: bool,

    /// Reflink options.
    ///
    /// Whether and how to use reflinks. 'auto' (the default) will
    /// attempt to reflink and fallback to a copy if it is not
    /// possible, 'always' will return an error if it cannot reflink,
    /// and 'never' will always perform a full data copy.
    ///
    /// Note: when using Linux accelerated copy operations (the
    /// default when available) the kernel may choose to reflink
    /// rather than perform a fully copy regardless of this setting.
    #[arg(long, default_value = "auto")]
    pub reflink: Reflink,

    /// Backup options
    ///
    /// Whether to create backups of overwritten files. Current
    /// options are 'none'/'off', or 'numbered', or 'auto'. Numbered
    /// backups follow the semantics of `cp` numbered backups
    /// (e.g. `file.txt.~123~`). 'auto' will only create a numbered
    /// backup if a previous backups exists. Default is 'none'.
    #[arg(long, default_value = "none")]
    pub backup: Backup,

    /// Path list.
    ///
    /// Source and destination files, or multiple source(s) to a directory.
    pub paths: Vec<String>,
}

impl Opts {
    pub fn from_args() -> Result<Opts> {
        Ok(Opts::parse())
    }

    pub fn log_level(&self) -> LevelFilter {
        match self.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

impl From<&Opts> for Config {
    fn from(opts: &Opts) -> Self {
        Config {
            workers: if opts.workers == 0 {
                num_cpus::get()
            } else {
                opts.workers
            },
            block_size: if opts.no_progress {
                usize::max_value() as u64
            } else {
                opts.block_size
            },
            gitignore: opts.gitignore,
            no_clobber: opts.no_clobber,
            no_perms: opts.no_perms,
            no_timestamps: opts.no_timestamps,
            dereference: opts.dereference,
            no_target_directory: opts.no_target_directory,
            fsync: opts.fsync,
            reflink: opts.reflink,
            backup: opts.backup,
        }
    }
}
