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

use std::fs::{create_dir_all, metadata, set_permissions, write, File};
use std::os::unix::fs::symlink;
use test_case::test_case;
use xattr;

mod util;
use crate::util::*;

#[test]
fn basic_help() {
    let out = run(&["--help"]).unwrap();

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Copy SOURCE to DEST"));
}

#[test]
fn no_args() {
    let out = run(&[]).unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("The following required arguments were not provided"));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn source_missing(drv: &str) {
    let out = run(&["--driver", drv, "/this/should/not/exist", "/dev/null"]).unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Source does not exist"));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn source_missing_globbed(drv: &str) {
    let out = run(&[
        "--driver",
        drv,
        "-g",
        "/this/should/not/exist/*.txt",
        "/dev/null",
    ]).unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("No source files found"));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path).unwrap();
        File::create(&dest_path).unwrap();
    }
    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_in_dir_exists(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path).unwrap();
        File::create(&dest_path).unwrap();
    }

    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists_overwrites(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa").unwrap();
        File::create(&dest_path).unwrap();
    }
    assert!(!files_match(&source_path, &dest_path));

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(files_match(&source_path, &dest_path));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists_noclobber(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa").unwrap();
        File::create(&dest_path).unwrap();
    }
    assert!(!files_match(&source_path, &dest_path));

    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
    assert!(!files_match(&source_path, &dest_path));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_perms(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    xattr::set(&source_path, "user.test", b"my test").unwrap();

    let mut perms = metadata(&source_path).unwrap().permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms).unwrap();


    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
    assert_eq!(
        metadata(&source_path).unwrap().permissions().readonly(),
        metadata(&dest_path).unwrap().permissions().readonly()
    );
    assert_eq!(xattr::get(&dest_path, "user.test").unwrap().unwrap(), b"my test");
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_no_perms(drv: &str) {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();
    let mut perms = metadata(&source_path).unwrap().permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--no-perms",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
    assert!(!metadata(&dest_path).unwrap().permissions().readonly());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_rel(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.join("source.txt");
    let dest_path = dir.join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_multiple(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-vv",
        f1.to_str().unwrap(),
        f2.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_empty_dir(drv: &str) {
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path).unwrap();

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir").exists());
    assert!(dest_base.join("mydir").is_dir());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_all_dirs(drv: &str) {
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path).unwrap();
    create_dir_all(source_path.join("one/two/three/")).unwrap();

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_all_dirs_rel(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    create_dir_all(&source_path).unwrap();
    create_dir_all(source_path.join("one/two/three/")).unwrap();

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_dirs_files(drv: &str) {
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path).unwrap();

    let mut p = source_path.clone();
    for d in ["one", "two", "three"].iter() {
        p.push(d);
        create_dir_all(&p).unwrap();
        create_file(&p.join(format!("{}.txt", d)), d).unwrap();
    }

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/one.txt").is_file());
    assert!(dest_base.join("mydir/one/two/two.txt").is_file());
    assert!(dest_base.join("mydir/one/two/three/three.txt").is_file());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
#[ignore] // Expensive so skip for local dev
fn copy_generated_tree(drv: &str) {
    let dir = tempdir().unwrap();

    let src = dir.path().join("generated");
    let dest = dir.path().join("target");

    // Spam some output to keep CI from timing-out (hopefully).
    println!("Generating file tree...");
    gen_filetree(&src, 0, false).unwrap();

    println!("Running copy...");
    let out = run(&[
        "--driver",
        drv,
        "-r",
        src.to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();
    assert!(out.status.success());

    println!("Compare trees...");
    compare_trees(&src, &dest).unwrap();
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_dirs_overwrites(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base).unwrap();
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig").unwrap());

    write(&source_file, "new content").unwrap();
    assert!(file_contains(&source_file, "new content").unwrap());

    out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "new content").unwrap());
    assert!(files_match(&source_file, &dest_file));
}

#[test]
/// when source path is a dir and target is a dir,
/// using the `-no-target-directory` flag will copy the source
/// dir _onto_ the target directory, instead of _under_ the
/// target dir (default behavior)
fn copy_dirs_overwrites_no_target_dir() {
    // First pass default behavior
    {
        let dir = tempdir_rel().unwrap();

        let source_path = dir.join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path).unwrap();
        create_file(&source_file, "orig").unwrap();

        let dest_base = dir.join("dest");
        create_dir_all(&dest_base).unwrap();
        let dest_file = dest_base.join("mydir/file.txt");

        let out = run(&[
            "-r",
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "orig").unwrap());
    }

    // Second pass `no target directory`
    {
        let dir = tempdir_rel().unwrap();

        let source_path = dir.join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path).unwrap();
        create_file(&source_file, "new content").unwrap();

        let dest_base = dir.join("dest");
        create_dir_all(&dest_base).unwrap();
        let dest_file = dest_base.join("file.txt");

        let out = run(&[
            "-r",
            "-T", //-no-target-directory
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ]).unwrap();

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "new content").unwrap());
    }
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_to_nonexistent_is_rename(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig").unwrap());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_overwrite_with_noclobber(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base).unwrap();
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig").unwrap());
    assert!(files_match(&source_file, &dest_file));

    write(&source_file, "new content").unwrap();
    assert!(file_contains(&source_file, "new content").unwrap());

    out = run(&[
        "--driver",
        drv,
        "-r",
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_containing_symlinks(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    let source_rlink = source_path.join("link.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();
    symlink("file.txt", source_rlink).unwrap();
    symlink("/etc/hosts", source_path.join("hosts")).unwrap();

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");
    let dest_rlink = source_path.join("link.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(dest_rlink.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(dest_base
        .join("hosts")
        .symlink_metadata().unwrap()
        .file_type()
        .is_symlink());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_with_hidden_dir(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir/.hidden");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.join("dest/.hidden");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig").unwrap());
    assert!(files_match(&source_file, &dest_file));
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_with_gitignore(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.join("mydir");
    create_dir_all(&source_path).unwrap();

    let source_file = source_path.join("file.txt");
    create_file(&source_file, "file content").unwrap();

    let ignore_file = source_path.join(".gitignore");
    create_file(&ignore_file, "/.ignored\n").unwrap();

    let ignored_path = dir.join("mydir/.ignored");
    let ignored_file = ignored_path.join("file.txt");
    create_dir_all(&ignored_path).unwrap();
    create_file(&ignored_file, "ignored content").unwrap();

    let hidden_path = dir.join("mydir/.hidden");
    let hidden_file = hidden_path.join("file.txt");
    create_dir_all(&hidden_path).unwrap();
    create_file(&hidden_file, "hidden content").unwrap();

    let dest_base = dir.join("dest");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        "--gitignore",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest_base.join("file.txt").exists());
    assert!(dest_base.join(".gitignore").exists());

    assert!(!dest_base.join(".ignored").exists());

    assert!(dest_base.join(".hidden").exists());
    assert!(dest_base.join(".hidden/file.txt").exists());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_with_glob(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.join("file*.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_pattern_no_glob(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.join("dest");
    create_dir_all(&dest).unwrap();

    let f1 = dir.join("a [b] c.txt");
    create_file(&f1, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        dir.join("a [b] c.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(out.status.success());
    assert!(dest.join("a [b] c.txt").exists());
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn glob_pattern_error(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.join("file***.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Pattern syntax error"));
}
