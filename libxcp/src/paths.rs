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

use std::path::Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::info;
use walkdir::DirEntry;

use crate::config::Config;
use crate::errors::Result;

/// Parse a git ignore file.
pub fn parse_ignore(source: &Path, config: &Config) -> Result<Option<Gitignore>> {
    let gitignore = if config.gitignore {
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

/// Filter to return whether a given file should be ignored by a
/// filter file.
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
