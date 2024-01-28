use std::{path::Path, env::current_dir};

use regex::Regex;

use crate::errors::{Result, XcpError};

const BAK_PATTTERN: &str = r"^~(\d+)~$";

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
    let cap = Regex::new(BAK_PATTTERN)
        .unwrap() // Fixed regex, so should never error.
        .captures(&suf)?;
    let num = cap.get(1)?
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
        { File::create(&base)?; }

        let next = next_backup_num(&base)?;
        assert_eq!(1, next);

        { File::create(dir.join("file.txt.~123~"))?; }

        let next = next_backup_num(&base)?;
        assert_eq!(124, next);

        Ok(())
    }

}
