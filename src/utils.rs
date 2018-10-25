use std::fs;
use std::result;
use std::path::PathBuf;

use glob::{glob, GlobResult, GlobError, Paths, PatternError};

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
        // Call glob() on each pattern, yealding Vec<Result<Paths>>.
        .map(|s| glob(&*s.as_str()))
        // Convert Vec<Result<>> to Result<Vec<>>.
        .collect::<result::Result<Vec<Paths>, _>>()?;
    let path_vecs = globs
        .iter_mut()
        // Force resolve each glob Paths iterator into a vector of the results...
        .map::<result::Result<Vec<PathBuf>, _>, _>(|p| p.collect())
        // And lift all the results up to the top.
        .collect::<result::Result<Vec<Vec<PathBuf>>,_>>()?;
    // And finally flatten the nested paths into a single collection of the results
    let paths = path_vecs
        .iter()
        .flat_map(|p| p.to_owned())
        .collect::<Vec<PathBuf>>();

    Ok(paths)
}
