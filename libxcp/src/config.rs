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

//! Driver configuration support.

use std::result;
use std::str::FromStr;

use crate::errors::XcpError;

/// Enum defining configuration options for handling
/// [reflinks](https://btrfs.readthedocs.io/en/latest/Reflink.html). [FromStr]
/// is supported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Reflink {
    /// Attempt to reflink and fallback to a copy if it is not
    /// possible.
    #[default]
    Auto,
    /// Always attempt a reflink; return an error if not supported.
    Always,
    /// Always perform a full data copy. Note: when using Linux
    /// accelerated copy operations (the default when available) the
    /// kernel may choose to reflink rather than perform a fully copy
    /// regardless of this setting.
    Never,
}

// String conversion helper as a convenience for command-line parsing.
impl FromStr for Reflink {
    type Err = XcpError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "always" => Ok(Reflink::Always),
            "auto" => Ok(Reflink::Auto),
            "never" => Ok(Reflink::Never),
            _ => Err(XcpError::InvalidArguments(format!("Unexpected value for 'reflink': {}", s))),
        }
    }
}

/// Enum defining configuration options for handling backups of
/// overwritten files. [FromStr] is supported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backup {
    /// Do not create backups.
    None,
    /// Create a numbered backup if a previous backup exists.
    Auto,
    /// Create numbered backups. Numbered backups follow the semantics
    /// of `cp` numbered backups (e.g. `file.txt.~123~`).
    Numbered
}

impl FromStr for Backup {
    type Err = XcpError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "off" => Ok(Backup::None),
            "auto" => Ok(Backup::Auto),
            "numbered" => Ok(Backup::Numbered),
            _ => Err(XcpError::InvalidArguments(format!("Unexpected value for 'backup': {}", s))),
        }
    }
}

/// A structure defining the runtime options for copy-drivers. This
/// would normally be passed to `load_driver()`.
#[derive(Clone, Debug)]
pub struct Config {
    /// Number of parallel workers. 0 means use the number of logical
    /// CPUs (the default).
    pub workers: usize,

    /// Block size for operations. Defaults to the full file size. Use
    /// a smaller value for finer-grained feedback.
    pub block_size: u64,

    /// Use .gitignore if present.
    ///
    /// NOTE: This is fairly basic at the moment, and only honours a
    /// .gitignore in the directory root for each source directory;
    /// global or sub-directory ignores are skipped. Default is
    /// `false`.
    pub gitignore: bool,

    /// Do not overwrite existing files. Default is `false`.
    pub no_clobber: bool,

    /// Do not copy the file permissions. Default is `false`.
    pub no_perms: bool,

    /// Target should not be a directory.
    ///
    /// Analogous to cp's no-target-directory. Expected behavior is that when
    /// copying a directory to another directory, instead of creating a sub-folder
    /// in target, overwrite target. Default is 'false`.
    pub no_target_directory: bool,

    /// Sync each file to disk after writing. Default is `false`.
    pub fsync: bool,

    /// Reflink options.
    ///
    /// Whether and how to use reflinks. 'auto' (the default) will
    /// attempt to reflink and fallback to a copy if it is not
    /// possible, 'always' will return an error if it cannot reflink,
    /// and 'never' will always perform a full data copy.
    pub reflink: Reflink,

    /// Backup options
    ///
    /// Whether to create backups of overwritten files. Current
    /// options are `None` or 'Numbered'. Numbered backups follow the
    /// semantics of `cp` numbered backups
    /// (e.g. `file.txt.~123~`). Default is `None`.
    pub backup: Backup,
}

impl Config {
    pub(crate) fn num_workers(&self) -> usize {
        if self.workers == 0 {
            num_cpus::get()
        } else {
            self.workers
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workers: num_cpus::get(),
            block_size: u64::max_value(),
            gitignore: false,
            no_clobber: false,
            no_perms: false,
            no_target_directory: false,
            fsync: false,
            reflink: Reflink::Auto,
            backup: Backup::None,
        }
    }
}
