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
    let suffix = format!(".~{}~", num);
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
        .ok_or(XcpError::InvalidArguments(format!("Invalid path found: {:?}", path)))?
        .to_string_lossy();
    Ok(fname.to_string())
}

fn has_backup(file: &Path) -> Result<bool> {
    let fname = filename(file)?;
    let exists = ls_file_dir(file)?
        .any(|der| if let Ok(de) = der {
            is_num_backup(&fname, &de.path()).is_some()
        } else {
            false
        });
    Ok(exists)
}

fn next_backup_num(file: &Path) -> Result<u64> {
    let fname = filename(file)?;
    let current = ls_file_dir(file)?
        .filter_map(|der| is_num_backup(&fname, &der.ok()?.path()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, fs::File};
    use tempfile::TempDir;

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

    #[test]
    fn test_needs_backup() -> Result<()> {
        let tdir = TempDir::new()?;
        let dir = tdir.path();
        let base = dir.join("file.txt");

        {
            File::create(&base)?;
        }
        assert!(!has_backup(&base)?);

        {
            File::create(dir.join("file.txt.~123~"))?;
        }
        assert!(has_backup(&base)?);

        {
            File::create(dir.join("file.txt.~999~"))?;
        }
        assert!(has_backup(&base)?);

        Ok(())
    }

}
