
use log::error;
use std::process;
use std::fs;

pub enum FileType {
    File,
    Dir,
    Symlink,
}

pub trait ToFileType {
    fn to_enum(&self) -> FileType;
}

impl ToFileType for fs::FileType {
    fn to_enum(&self) -> FileType {
        if self.is_dir() {
            FileType::Dir
        } else if self.is_file() {
            FileType::File
        } else if self.is_symlink() {
            FileType::Symlink
        } else {
            error!("Unknown filetype found; this should never happen!");
            process::abort()
        }
    }
}
