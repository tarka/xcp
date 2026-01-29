use std::{
    path::{Path, PathBuf},
    env::current_dir, sync::OnceLock, fs::ReadDir,
};

use regex::Regex;

use crate::{errors::{Result, XcpError}, config::{Config, Backup}};

const BAK_PATTTERN: &str = r"^\~(\d+)\~$";
static BAK_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_regex() -> &'static Regex {
    // Fixed regex, so should never error.
    BAK_REGEX.get_or_init(|| Regex::new(BAK_PATTTERN).unwrap())
}

pub(crate) fn get_backup_path(file: &Path) -> Result<PathBuf> {
    let num = next_backup_num(file)?;
    let suffix = format!(".~{num}~");
    // Messy but PathBuf has no concept of mulitiple extensions.
    let mut bstr = file.to_path_buf().into_os_string();
    bstr.push(suffix);
    let backup = PathBuf::from(bstr);
    Ok(backup)
}

pub(crate) fn needs_backup(file: &Path, conf: &Config) -> Result<bool> {
    let need = match conf.backup {
        Backup::None => false,
        Backup::Auto if file.exists() => {
            has_backup(file)?
        }
        Backup::Numbered if file.exists() => true,
        _ => false,
    };
    Ok(need)
}

fn ls_file_dir(file: &Path) -> Result<ReadDir> {
    let cwd = current_dir()?;
    let ls_dir = file.parent()
        .map(|p| if p.as_os_str().is_empty() {
            &cwd
        } else {
            p
        })
        .unwrap_or(&cwd)
        .read_dir()?;
    Ok(ls_dir)
}

fn filename(path: &Path) -> Result<String> {
    let fname = path.file_name()
        .ok_or(XcpError::InvalidArguments(format!("Invalid path found: {path:?}")))?
        .to_string_lossy();
    Ok(fname.to_string())
}

pub(crate) fn has_backup(file: &Path) -> Result<bool> {
    let fname = filename(file)?;
    let exists = ls_file_dir(file)?
        .any(|der| if let Ok(de) = der {
            is_num_backup(&fname, &de.path()).is_some()
        } else {
            false
        });
    Ok(exists)
}

pub(crate) fn next_backup_num(file: &Path) -> Result<u64> {
    let fname = filename(file)?;
    let current = ls_file_dir(file)?
        .filter_map(|der| is_num_backup(&fname, &der.ok()?.path()))
        .max()
        .unwrap_or(0);
    Ok(current + 1)
}

pub(crate) fn is_num_backup(base_file: &str, candidate: &Path) -> Option<u64> {
    let cname = candidate
        .file_name()?
        .to_str()?;
    if !cname.starts_with(base_file) {
        return None
    }
    let ext = candidate
        .extension()?
        .to_string_lossy();
    let num = get_regex()
        .captures(&ext)?
        .get(1)?
        .as_str()
        .parse::<u64>()
        .ok()?;
    Some(num)
}
