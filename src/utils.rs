
use std::fs::{FileType};

pub enum EFileType {
    File,
    Dir,
    Symlink,
    Unsupported,
}

pub trait ToEnum {
    fn to_enum(&self) -> EFileType;
}

impl ToEnum for FileType {
    fn to_enum(&self) -> EFileType {
        if self.is_dir() {
            EFileType::Dir
        } else if self.is_file() {
            EFileType::File
        } else if self.is_symlink() {
            EFileType::Symlink
        } else {
            EFileType::Unsupported
        }
    }
}
