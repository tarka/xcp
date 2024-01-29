use std::{
    path::{Path, PathBuf},
    env::current_dir, sync::OnceLock,
};

use regex::Regex;

use crate::errors::{Result, XcpError};

const BAK_PATTTERN: &str = r"^\~(\d+)\~$";
static BAK_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_regex() -> &'static Regex {
    // Fixed regex, so should never error.
    BAK_REGEX.get_or_init(|| Regex::new(BAK_PATTTERN).unwrap())
}

pub(crate) fn get_backup_path(file: &Path) -> Result<PathBuf> {
    let num = next_backup_num(file)?;
    let suffix = format!(".~{}~", num);
    // Messy but PathBuf has no concept of mulitiple extensions.
    let mut bstr = file.to_path_buf().into_os_string();
    bstr.push(suffix);
    let backup = PathBuf::from(bstr);
    Ok(backup)
}


fn next_backup_num(file: &Path) -> Result<u64> {
    let fname = file.file_name()
        .ok_or(XcpError::InvalidArguments(format!("Invalid path found: {:?}", file)))?
        .to_string_lossy();
    let cwd = current_dir()?;
    let current = file.parent()
        .map(|p| if p.as_os_str().is_empty() {
            &cwd
        } else {
            p
        })
        .unwrap_or(&cwd)
        .read_dir()?
        .filter_map(|de| is_num_backup(&fname, &de.ok()?.path()))
        .max()
        .unwrap_or(0);
    Ok(current + 1)
}

fn is_num_backup(base_file: &str, candidate: &Path) -> Option<u64> {
    let cname = candidate
        .file_name()?
        .to_str()?;
    if !cname.starts_with(base_file) {
        return None
    }
    let suf = candidate.extension()?
        .to_string_lossy();
    let num = get_regex()
        .captures(&suf)?
        .get(1)?
        .as_str()
        .parse::<u64>()
        .ok()?;
    Some(num)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, fs::File};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_is_backup() {
        let cand = PathBuf::from("/some/path/file.txt.~123~");

        let bnum = is_num_backup("file.txt", &cand);
        assert!(bnum.is_some());
        assert_eq!(123, bnum.unwrap());

        let bnum = is_num_backup("other_file.txt", &cand);
        assert!(bnum.is_none());

        let bnum = is_num_backup("le.txt", &cand);
        assert!(bnum.is_none());
    }

    #[test]
    fn test_backup_num_scan() -> Result<()> {
        let tdir = TempDir::new()?;
        let dir = tdir.path();
        let base = dir.join("file.txt");

        {
            File::create(&base)?;
        }
        let next = next_backup_num(&base)?;
        assert_eq!(1, next);

        {
            File::create(dir.join("file.txt.~123~"))?;
        }
        let next = next_backup_num(&base)?;
        assert_eq!(124, next);

        {
            File::create(dir.join("file.txt.~999~"))?;
        }
        let next = next_backup_num(&base)?;
        assert_eq!(1000, next);

        Ok(())
    }

    #[test]
    fn test_gen_backup_path() -> Result<()> {
        let tdir = TempDir::new()?;
        let dir = tdir.path();
        let base = dir.join("file.txt");
        {
            File::create(&base)?;
        }

        let backup = get_backup_path(&base)?;
        let mut bs = base.into_os_string();
        bs.push(".~1~");
        assert_eq!(PathBuf::from(bs), backup);

        Ok(())
    }

}
