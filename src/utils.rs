use std::fs;

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
