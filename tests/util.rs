#![allow(unused)]
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

use anyhow::{self, Error};
use fslock::LockFile;
use rand::{Rng, RngCore, SeedableRng, thread_rng};
use rand_distr::{Alphanumeric, Pareto, Triangular, Standard};
use rand_xorshift::XorShiftRng;
use std::cmp;
use std::env::current_dir;
use std::fs::{create_dir_all, File, FileTimes};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::result;
use std::time::{Duration, SystemTime};
use tempfile::{tempdir_in, TempDir};
use uuid::Uuid;
use walkdir::WalkDir;

pub type TResult = result::Result<(), Error>;

pub fn get_command() -> Result<Command, Error> {
    let exe = env!("CARGO_BIN_EXE_xcp");
    Ok(Command::new(exe))
}

pub fn run(args: &[&str]) -> Result<Output, Error> {
    let out = get_command()?.args(args).output()?;
    println!("STDOUT: {}", String::from_utf8_lossy(&out.stdout));
    println!("STDERR: {}", String::from_utf8_lossy(&out.stderr));
    Ok(out)
}

pub fn tempdir_rel() -> Result<TempDir, Error> {
    // let uuid = Uuid::new_v4();
    // let dir = PathBuf::from("target/").join(uuid.to_string());
    // create_dir_all(&dir)?;
    // Ok(dir)
    Ok(tempdir_in(current_dir()?.join("target"))?)
}

pub fn create_file(path: &Path, text: &str) -> Result<(), Error> {
    let file = File::create(path)?;
    write!(&file, "{}", text)?;
    Ok(())
}

pub fn set_time_past(file: &Path) -> Result<(), Error> {
    let yr = Duration::from_secs(60 * 60 * 24 * 365);
    let past = SystemTime::now().checked_sub(yr).unwrap();
    let ft = FileTimes::new()
        .set_modified(past);
    File::open(file)?.set_times(ft)?;
    Ok(())
}

pub fn timestamps_same(from: &SystemTime, to: &SystemTime) -> bool {
    let from_s = from.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
    let to_s = to.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
    // 5s tolerance
    from_s.abs_diff(to_s) < 5
}


#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(unused)]
pub fn create_sparse(file: &Path, head: u64, tail: u64) -> Result<u64, Error> {
    let data = "c00lc0d3";
    let len = 4096u64 * 4096 + data.len() as u64 + tail;

    let out = Command::new("/usr/bin/truncate")
        .args(["-s", len.to_string().as_str(), file.to_str().unwrap()])
        .output()?;
    assert!(out.status.success());

    let mut fd = std::fs::OpenOptions::new()
        .write(true)
        .append(false)
        .open(file)?;

    fd.seek(SeekFrom::Start(head))?;
    write!(fd, "{}", data)?;

    fd.seek(SeekFrom::Start(1024 * 4096))?;
    write!(fd, "{}", data)?;

    fd.seek(SeekFrom::Start(4096 * 4096))?;
    write!(fd, "{}", data)?;

    Ok(len)
}

#[allow(unused)]
pub fn file_contains(path: &Path, text: &str) -> Result<bool, Error> {
    let mut dest = File::open(path)?;
    let mut buf = String::new();
    dest.read_to_string(&mut buf)?;

    Ok(buf == text)
}

pub fn files_match(a: &Path, b: &Path) -> bool {
    println!("Checking: {:?}", a);
    if a.metadata().unwrap().len() != b.metadata().unwrap().len() {
        return false;
    }
    let mut abr = BufReader::with_capacity(1024 * 1024, File::open(a).unwrap());
    let mut bbr = BufReader::with_capacity(1024 * 1024, File::open(b).unwrap());
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
        let dir = tempdir_rel()?;
        let a = dir.path().join("source.txt");
        let b = dir.path().join("dest.txt");
        let text = "sd;lkjfasl;kjfa;sldkfjaslkjfa;jsdlfkjsdlfkajl";
        create_file(&a, text)?;
        create_file(&b, text)?;
        assert!(files_match(&a, &b));
    }
    {
        let dir = tempdir_rel()?;
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
        .args(["--format", "%s %b %B", file.to_str().unwrap()])
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
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn probably_sparse(file: &Path) -> Result<bool, Error> {
    Ok(false)
}

pub fn rand_data(len: usize) -> Vec<u8> {
    thread_rng()
        .sample_iter(Standard)
        .take(len)
        .collect()
}

const MAXDEPTH: u64 = 2;

pub fn gen_file_name(rng: &mut dyn RngCore, len: u64) -> String {
    let r = rng
        .sample_iter(Alphanumeric)
        .take(len as usize)
        .collect::<Vec<u8>>();
    String::from_utf8(r).unwrap()
}

pub fn gen_file(path: &Path, rng: &mut dyn RngCore, size: usize, sparse: bool) -> TResult {
    println!("Generating: {:?}", path);
    let mut fd = File::create(path)?;
    const BSIZE: usize = 4096;
    let mut buffer = [0; BSIZE];
    let mut left = size;

    while left > 0 {
        let blen = cmp::min(left, BSIZE);
        let b = &mut buffer[..blen];
        rng.fill(b);
        if sparse && b[0] % 3 == 0 {
            fd.seek(SeekFrom::Current(blen as i64))?;
            left -= blen;
        } else {
            left -= fd.write(b)?;
        }
    }

    Ok(())
}

/// Recursive random file-tree generator. The distributions have been
/// manually chosen to give a rough approximation of a working
/// project, with most files in the 10's of Ks, and a few larger
/// ones. With a seeded PRNG (see below) this will give a repeatable
/// tree depending on the seed.
pub fn gen_subtree(base: &Path, rng: &mut dyn RngCore, depth: u64, with_sparse: bool) -> TResult {
    create_dir_all(base)?;

    let dist0 = Triangular::new(0.0, 64.0, 64.0 / 5.0)?;
    let dist1 = Triangular::new(1.0, 64.0, 64.0 / 5.0)?;
    let distf = Pareto::new(50.0 * 1024.0, 1.0)?;

    let nfiles = rng.sample(dist0) as u64;
    for _ in 0..nfiles {
        let fnlen = rng.sample(dist1) as u64;
        let fsize = rng.sample(distf) as u64;
        let fname = gen_file_name(rng, fnlen);
        let path = base.join(fname);
        let sparse = with_sparse && nfiles % 3 == 0;
        gen_file(&path, rng, fsize as usize, sparse)?;
    }

    if depth < MAXDEPTH {
        let ndirs = rng.sample(dist1) as u64;
        for _ in 0..ndirs {
            let fnlen = rng.sample(dist1) as u64;
            let fname = gen_file_name(rng, fnlen);
            let path = base.join(fname);
            gen_subtree(&path, rng, depth + 1, with_sparse)?;
        }
    }

    Ok(())
}

pub fn gen_global_filetree(with_sparse: bool) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from("target/generated_filetree");
    let lockfile = path.with_extension("lock");

    let mut lf = LockFile::open(&lockfile)?;
    lf.lock()?;
    if !path.exists() {
        gen_filetree(&path, 0, with_sparse)?;
    }
    lf.unlock();

    Ok(path)
}

pub fn gen_filetree(base: &Path, seed: u64, with_sparse: bool) -> TResult {
    let mut rng = XorShiftRng::seed_from_u64(seed);
    gen_subtree(base, &mut rng, 0, with_sparse)
}

pub fn compare_trees(src: &Path, dest: &Path) -> TResult {
    let pref = src.components().count();
    for entry in WalkDir::new(src) {
        let from = entry?.into_path();
        let tail: PathBuf = from.components().skip(pref).collect();
        let to = dest.join(tail);

        assert!(to.exists());
        assert_eq!(from.is_dir(), to.is_dir());
        assert_eq!(
            from.metadata()?.file_type().is_symlink(),
            to.metadata()?.file_type().is_symlink()
        );

        if from.is_file() {
            assert_eq!(probably_sparse(&to)?, probably_sparse(&to)?);
            assert!(files_match(&from, &to));
            // FIXME: Ideally we'd check sparse holes here, but
            // there's no guarantee they'll match exactly due to
            // low-level filesystem details (SEEK_HOLE behaviour,
            // tail-packing, compression, etc.)
        }
    }
    Ok(())
}
