/*
 * Copyright © 2018, Steve Smith <tarkasteve@gmail.com>
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

use anyhow::Error;
use core::hash::Hasher;
use escargot::CargoBuild;
use fxhash::FxHasher64;
use rand::{Rng, RngCore, SeedableRng};
use rand_distr::{Alphanumeric, Pareto, Triangular};
use rand_xorshift::XorShiftRng;
use std::cmp;
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::result;
use tempfile::tempdir;
use uuid::Uuid;
use walkdir::WalkDir;


pub type TResult = result::Result<(), Error>;

pub fn get_command() -> Result<Command, Error> {
    let cmd = CargoBuild::new().run()?.command();
    Ok(cmd)
}

pub fn run(args: &[&str]) -> Result<Output, Error> {
    let out = get_command()?.args(args).output()?;
    Ok(out)
}

pub fn tempdir_rel() -> Result<PathBuf, Error> {
    let uuid = Uuid::new_v4();
    let dir = PathBuf::from("target/").join(uuid.to_string());
    create_dir_all(&dir)?;
    Ok(dir)
}

pub fn create_file(path: &Path, text: &str) -> Result<(), Error> {
    let file = File::create(&path)?;
    write!(&file, "{}", text)?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn create_sparse(file: &Path, head: u64, tail: u64) -> Result<u64, Error> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom};

    let data = "c00lc0d3";
    let len = 4096u64 * 4096 + data.len() as u64 + tail;

    let out = Command::new("/usr/bin/truncate")
        .args(&["-s", len.to_string().as_str(),
                file.to_str().unwrap()])
        .output()?;
    assert!(out.status.success());

    let mut fd = OpenOptions::new()
        .write(true)
        .append(false)
        .open(&file)?;

    fd.seek(SeekFrom::Start(head))?;
    write!(fd, "{}", data)?;

    fd.seek(SeekFrom::Start(1024*4096))?;
    write!(fd, "{}", data)?;

    fd.seek(SeekFrom::Start(4096*4096))?;
    write!(fd, "{}", data)?;

    Ok(len as u64)
}

pub fn file_contains(path: &Path, text: &str) -> Result<bool, Error> {
    let mut dest = File::open(path)?;
    let mut buf = String::new();
    dest.read_to_string(&mut buf)?;

    Ok(buf == text)
}

pub fn files_match(a: &Path, b: &Path) -> bool {
    println!("CHECKING {:?}", a);
    if a.metadata().unwrap().len() != b.metadata().unwrap().len() {
        return false;
    }
    // let ah = hash_file(a).unwrap();
    // let bh = hash_file(b).unwrap();
    let mut abr = BufReader::with_capacity(1024*1024, File::open(a).unwrap());
    let mut bbr = BufReader::with_capacity(1024*1024, File::open(b).unwrap());
    loop {
        let read = {
            let ab = abr.fill_buf().unwrap();
            let bb = bbr.fill_buf().unwrap();
            if ab != bb {
                return false;
            }
            if ab.is_empty() {
                return true;
            }
            ab.len()
        };
        abr.consume(read);
        bbr.consume(read);
    }
}


#[test]
fn test_hasher() -> TResult {
    {
        let dir = tempdir()?;
        let a = dir.path().join("source.txt");
        let b = dir.path().join("dest.txt");
        let text = "sd;lkjfasl;kjfa;sldkfjaslkjfa;jsdlfkjsdlfkajl";
        create_file(&a, text)?;
        create_file(&b, text)?;
        assert!(files_match(&a, &b));
    }
    {
        let dir = tempdir()?;
        let a = dir.path().join("source.txt");
        let b = dir.path().join("dest.txt");
        create_file(&a, "lskajdf;laksjdfl;askjdf;alksdj")?;
        create_file(&b, "29483793857398")?;
        assert!(!files_match(&a, &b));
    }

    Ok(())
}


#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn quickstat(file: &Path) -> Result<(i32, i32, i32), Error> {
    let out = Command::new("stat")
        .args(&["--format", "%s %b %B",
                file.to_str().unwrap()])
        .output()?;
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout)?;
    let stats = stdout
        .split_whitespace()
        .map(|s| s.parse::<i32>().unwrap())
        .collect::<Vec<i32>>();
    let (size, blocks, blksize) = (stats[0], stats[1], stats[2]);

    Ok((size, blocks, blksize))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn probably_sparse(file: &Path) -> Result<bool, Error> {
    let (size, blocks, blksize) = quickstat(file)?;

    Ok(blocks < size / blksize)
}


const MAXDEPTH: u64 = 2;

pub fn gen_file_name(rng: &mut dyn RngCore, len: u64) -> String {
    rng.sample_iter(Alphanumeric)
        .take(len as usize)
        .collect()
}

pub fn gen_file(path: &Path, rng: &mut dyn RngCore, size: usize) -> TResult {
    let mut fd = File::create(path)?;
    let mut buffer = [0; 1024*1024];
    let mut left = size;

    while left > 0 {
        let b = &mut buffer[..cmp::min(left, 1024*1024)];
        rng.fill(b);
        left -= fd.write(b)?;
    }

    Ok(())
}

/// Recursive random file-tree generator. The distributions have been
/// manually chosen to give a rough approximation of a working
/// project, with most files in the 10's of Ks, and a few larger
/// ones. With a seeded PRNG (see below) this will give a repeatable
/// tree depending on the seed.
pub fn gen_subtree(base: &Path, rng: &mut dyn RngCore, depth: u64) -> TResult {
    create_dir_all(base)?;

    let dist0 = Triangular::new(0.0, 64.0, 64.0/5.0).unwrap();
    let dist1 = Triangular::new(1.0, 64.0, 64.0/5.0).unwrap();
    let distf = Pareto::new(50.0*1024.0, 1.0).unwrap();

    let nfiles = rng.sample(dist0) as u64;
    for _ in 0..nfiles {
        let fnlen = rng.sample(dist1) as u64;
        let fsize = rng.sample(distf) as u64;
        let fname = gen_file_name(rng, fnlen);
        let path = base.join(fname);
        gen_file(&path, rng, fsize as usize)?;
    }

    if depth < MAXDEPTH {
        let ndirs = rng.sample(dist1) as u64;
        for _ in 0..ndirs {
            let fnlen = rng.sample(dist1) as u64;
            let fname = gen_file_name(rng, fnlen);
            let path = base.join(fname);
            gen_subtree(&path, rng, depth+1)?;
        }
    }

    Ok(())
}

pub fn gen_filetree(base: &Path, seed: u64) -> TResult {
    let mut rng = XorShiftRng::seed_from_u64(seed);
    gen_subtree(base, &mut rng, 0)?;
    Ok(())
}

pub fn compare_trees(src: &Path, dest: &Path) -> TResult {
    let pref = src.components().count();
    for entry in WalkDir::new(src) {
        let from = entry?.into_path();
        let tail: PathBuf = from.components().skip(pref).collect();
        let to = dest.join(tail);

        assert!(to.exists());
        assert_eq!(from.is_dir(), to.is_dir());
        assert_eq!(from.metadata()?.file_type().is_symlink(),
                   to.metadata()?.file_type().is_symlink());

        if from.is_file() {
            assert!(files_match(&from, &to));
        }

        // FIXME: Check sparse holes.
    }
    Ok(())
}
