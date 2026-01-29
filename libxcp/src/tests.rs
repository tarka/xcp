


use std::{path::PathBuf, fs::File};
use tempfile::TempDir;

use std::sync::Arc;
use std::thread;

use crate::backup::{get_backup_path, has_backup, is_num_backup, next_backup_num};
use crate::errors::{Result, XcpError};
use crate::config::Config;
use crate::feedback::{ChannelUpdater, StatusUpdater, StatusUpdate};
use crate::drivers::{Drivers, load_driver};

#[test]
fn simple_usage_test() -> Result<()> {
    let sources = vec![PathBuf::from("src")];
    let dest = TempDir::new()?;

    let config = Arc::new(Config::default());
    let updater = ChannelUpdater::new(&config);
    let stat_rx = updater.rx_channel();
    let stats: Arc<dyn StatusUpdater> = Arc::new(updater);

    let driver = load_driver(Drivers::ParFile, &config)?;

    let handle = thread::spawn(move || {
        driver.copy(sources, dest.path(), stats)
    });

    // Gather the results as we go; our end of the channel has been
    // moved to the driver call and will end when drained.
    for stat in stat_rx {
        match stat {
            StatusUpdate::Copied(v) => {
                println!("Copied {v} bytes");
            },
            StatusUpdate::Size(v) => {
                println!("Size update: {v}");
            },
            StatusUpdate::Error(e) => {
                println!("Error during copy: {e}");
                return Err(e.into());
            }
        }
    }

    handle.join()
        .map_err(|_| XcpError::CopyError("Error during copy operation".to_string()))??;

    println!("Copy complete");

    Ok(())
}


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
