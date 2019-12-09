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

mod util;

use std::fs::{create_dir_all, metadata, set_permissions, write, File};
use std::os::unix::fs::symlink;
use std::process::Command;
use tempfile::tempdir;
use test_case::test_case;

use crate::util::*;

#[test]
fn basic_help() -> TResult {
    let out = run(&["--help"])?;

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout)?;
    assert!(stdout.contains("Copy SOURCE to DEST"));

    Ok(())
}

#[test]
fn no_args() -> TResult {
    let out = run(&[])?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("The following required arguments were not provided"));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn source_missing(drv: &str) -> TResult {
    let out = run(&["--driver", drv, "/this/should/not/exist", "/dev/null"])?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Source does not exist"));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn source_missing_globbed(drv: &str) -> TResult {
    let out = run(&[
        "--driver",
        drv,
        "-g",
        "/this/should/not/exist/*.txt",
        "/dev/null",
    ])?;

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("No source files found"));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path)?;
        File::create(&dest_path)?;
    }
    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_in_dir_exists(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path)?;
        File::create(&dest_path)?;
    }

    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists_overwrites(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa")?;
        File::create(&dest_path)?;
    }
    assert!(!files_match(&source_path, &dest_path));

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(files_match(&source_path, &dest_path));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dest_file_exists_noclobber(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa")?;
        File::create(&dest_path)?;
    }
    assert!(!files_match(&source_path, &dest_path));

    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Destination file exists"));
    assert!(!files_match(&source_path, &dest_path));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);
    assert!(files_match(&source_path, &dest_path));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_perms(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;
    let mut perms = metadata(&source_path)?.permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms)?;

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);
    assert!(files_match(&source_path, &dest_path));
    assert_eq!(
        metadata(&source_path)?.permissions().readonly(),
        metadata(&dest_path)?.permissions().readonly()
    );

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_no_perms(drv: &str) -> TResult {
    let dir = tempdir()?;
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;
    let mut perms = metadata(&source_path)?.permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms)?;

    let out = run(&[
        "--driver",
        drv,
        "--no-perms",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);
    assert!(files_match(&source_path, &dest_path));
    assert!(!metadata(&dest_path)?.permissions().readonly());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_rel(drv: &str) -> TResult {
    let dir = tempdir_rel()?;
    let source_path = dir.join("source.txt");
    let dest_path = dir.join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text)?;

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text)?);
    assert!(files_match(&source_path, &dest_path));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn file_copy_multiple(drv: &str) -> TResult {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test")?;
    create_file(&f2, "test")?;

    let out = run(&[
        "--driver",
        drv,
        "-vv",
        f1.to_str().unwrap(),
        f2.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_empty_dir(drv: &str) -> TResult {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir").exists());
    assert!(dest_base.join("mydir").is_dir());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_all_dirs(drv: &str) -> TResult {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;
    create_dir_all(source_path.join("one/two/three/"))?;

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_all_dirs_rel(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    create_dir_all(&source_path)?;
    create_dir_all(source_path.join("one/two/three/"))?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_dirs_files(drv: &str) -> TResult {
    let dir = tempdir()?;

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path)?;

    let mut p = source_path.clone();
    for d in ["one", "two", "three"].iter() {
        p.push(d);
        create_dir_all(&p)?;
        create_file(&p.join(format!("{}.txt", d)), d)?;
    }

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base)?;

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/one.txt").is_file());
    assert!(dest_base.join("mydir/one/two/two.txt").is_file());
    assert!(dest_base.join("mydir/one/two/three/three.txt").is_file());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
#[ignore] // Expensive so skip for local dev
fn copy_generated_tree(drv: &str) -> TResult {
    let dir = tempdir()?;

    let src = dir.path().join("generated");
    let dest = dir.path().join("target");

    // Spam some output to keep CI from timing-out (hopefully).
    println!("Generating file tree...");
    gen_filetree(&src, 0, false)?;

    println!("Running copy...");
    let out = run(&[
        "--driver",
        drv,
        "-r",
        src.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;
    assert!(out.status.success());

    println!("Compare trees...");
    compare_trees(&src, &dest)?;

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
#[ignore] // Expensive so skip for local dev
fn copy_generated_tree_sparse(drv: &str) -> TResult {
    let dir = tempdir()?;

    let src = dir.path().join("generated");
    let dest = dir.path().join("target");

    // Spam some output to keep CI from timing-out (hopefully).
    println!("Generating file tree...");
    gen_filetree(&src, 0, true)?;

    println!("Running copy...");
    let out = run(&[
        "--driver",
        drv,
        "-r",
        src.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;
    assert!(out.status.success());

    println!("Compare trees...");
    compare_trees(&src, &dest)?;

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_dirs_overwrites(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig")?);

    write(&source_file, "new content")?;
    assert!(file_contains(&source_file, "new content")?);

    out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "new content")?);
    assert!(files_match(&source_file, &dest_file));

    Ok(())
}

#[test]
/// when source path is a dir and target is a dir,
/// using the `-no-target-directory` flag will copy the source
/// dir _onto_ the target directory, instead of _under_ the
/// target dir (default behavior)
fn copy_dirs_overwrites_no_target_dir() -> TResult {
    // First pass default behavior
    {
        let dir = tempdir_rel()?;

        let source_path = dir.join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path)?;
        create_file(&source_file, "orig")?;

        let dest_base = dir.join("dest");
        create_dir_all(&dest_base)?;
        let dest_file = dest_base.join("mydir/file.txt");

        let out = run(&[
            "-r",
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ])?;

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "orig")?);
    }

    // Second pass `no target directory`
    {
        let dir = tempdir_rel()?;

        let source_path = dir.join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path)?;
        create_file(&source_file, "new content")?;

        let dest_base = dir.join("dest");
        create_dir_all(&dest_base)?;
        let dest_file = dest_base.join("file.txt");

        let out = run(&[
            "-r",
            "-T", //-no-target-directory
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ])?;

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "new content")?);
    }

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_to_nonexistent_is_rename(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_overwrite_with_noclobber(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    create_dir_all(&dest_base)?;
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig")?);
    assert!(files_match(&source_file, &dest_file));

    write(&source_file, "new content")?;
    assert!(file_contains(&source_file, "new content")?);

    out = run(&[
        "--driver",
        drv,
        "-r",
        "--no-clobber",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_containing_symlinks(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join("file.txt");
    let source_rlink = source_path.join("link.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;
    symlink("file.txt", source_rlink)?;
    symlink("/etc/hosts", source_path.join("hosts"))?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join("file.txt");
    let dest_rlink = source_path.join("link.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(dest_rlink.symlink_metadata()?.file_type().is_symlink());
    assert!(dest_base
        .join("hosts")
        .symlink_metadata()?
        .file_type()
        .is_symlink());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_with_hidden_file(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    let source_file = source_path.join(".file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest");
    let dest_file = dest_base.join(".file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);
    assert!(files_match(&source_file, &dest_file));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_copy_with_hidden_dir(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir/.hidden");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path)?;
    create_file(&source_file, "orig")?;

    let dest_base = dir.join("dest/.hidden");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig")?);
    assert!(files_match(&source_file, &dest_file));

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn dir_with_gitignore(drv: &str) -> TResult {
    let dir = tempdir_rel()?;

    let source_path = dir.join("mydir");
    create_dir_all(&source_path)?;

    let source_file = source_path.join("file.txt");
    create_file(&source_file, "file content")?;

    let ignore_file = source_path.join(".gitignore");
    create_file(&ignore_file, "/.ignored\n")?;

    let ignored_path = dir.join("mydir/.ignored");
    let ignored_file = ignored_path.join("file.txt");
    create_dir_all(&ignored_path)?;
    create_file(&ignored_file, "ignored content")?;

    let hidden_path = dir.join("mydir/.hidden");
    let hidden_file = hidden_path.join("file.txt");
    create_dir_all(&hidden_path)?;
    create_file(&hidden_file, "hidden content")?;

    let dest_base = dir.join("dest");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        "--gitignore",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest_base.join("file.txt").exists());
    assert!(dest_base.join(".gitignore").exists());

    assert!(!dest_base.join(".ignored").exists());

    assert!(dest_base.join(".hidden").exists());
    assert!(dest_base.join(".hidden/file.txt").exists());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_with_glob(drv: &str) -> TResult {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test")?;
    create_file(&f2, "test")?;

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.join("file*.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn copy_pattern_no_glob(drv: &str) -> TResult {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let f1 = dir.join("a [b] c.txt");
    create_file(&f1, "test")?;

    let out = run(&[
        "--driver",
        drv,
        dir.join("a [b] c.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(dest.join("a [b] c.txt").exists());

    Ok(())
}

#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn glob_pattern_error(drv: &str) -> TResult {
    let dir = tempdir_rel()?;
    let dest = dir.join("dest");
    create_dir_all(&dest)?;

    let (f1, f2) = (dir.join("file1.txt"), dir.join("file2.txt"));
    create_file(&f1, "test")?;
    create_file(&f2, "test")?;

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.join("file***.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])?;

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr)?;
    assert!(stderr.contains("Pattern syntax error"));

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn test_sparse(drv: &str) -> TResult {
    use std::fs::read;

    let dir = tempdir_rel()?;
    let from = dir.join("sparse.bin");
    let to = dir.join("target.bin");

    let slen = create_sparse(&from, 0, 0)?;
    assert_eq!(slen, from.metadata()?.len());
    assert!(probably_sparse(&from)?);

    let out = run(&[
        "--driver",
        drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ])?;
    assert!(out.status.success());

    assert!(probably_sparse(&to)?);

    assert_eq!(quickstat(&from)?, quickstat(&to)?);

    let from_data = read(&from)?;
    let to_data = read(&to)?;
    assert_eq!(from_data, to_data);

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn test_sparse_leading_gap(drv: &str) -> TResult {
    use std::fs::read;

    let dir = tempdir()?;
    let from = dir.path().join("sparse.bin");
    let to = dir.path().join("target.bin");

    let slen = create_sparse(&from, 1024, 0)?;
    assert_eq!(slen, from.metadata()?.len());
    assert!(probably_sparse(&from)?);

    let out = run(&[
        "--driver",
        drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ])?;

    assert!(out.status.success());
    assert!(probably_sparse(&to)?);
    assert_eq!(quickstat(&from)?, quickstat(&to)?);

    let from_data = read(&from)?;
    let to_data = read(&to)?;
    assert_eq!(from_data, to_data);

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn test_sparse_trailng_gap(drv: &str) -> TResult {
    use std::fs::read;

    let dir = tempdir()?;
    let from = dir.path().join("sparse.bin");
    let to = dir.path().join("target.bin");

    let slen = create_sparse(&from, 1024, 1024)?;
    assert_eq!(slen, from.metadata()?.len());
    assert!(probably_sparse(&from)?);

    let out = run(&[
        "--driver",
        drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ])?;
    assert!(out.status.success());

    assert!(probably_sparse(&to)?);
    assert_eq!(quickstat(&from)?, quickstat(&to)?);

    let from_data = read(&from)?;
    let to_data = read(&to)?;
    assert_eq!(from_data, to_data);

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn test_sparse_single_overwrite(drv: &str) -> TResult {
    use std::fs::read;

    let dir = tempdir()?;
    let from = dir.path().join("sparse.bin");
    let to = dir.path().join("target.bin");

    let slen = create_sparse(&from, 1024, 1024)?;
    create_file(&to, "")?;
    assert_eq!(slen, from.metadata()?.len());
    assert!(probably_sparse(&from)?);

    let out = run(&[
        "--driver",
        drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ])?;
    assert!(out.status.success());
    assert!(probably_sparse(&to)?);
    assert_eq!(quickstat(&from)?, quickstat(&to)?);

    let from_data = read(&from)?;
    let to_data = read(&to)?;
    assert_eq!(from_data, to_data);

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[test_case("parblock"; "Test with parallel block driver")]
fn test_empty_sparse(drv: &str) -> TResult {
    use std::fs::read;

    let dir = tempdir()?;
    let from = dir.path().join("sparse.bin");
    let to = dir.path().join("target.bin");

    let out = Command::new("/usr/bin/truncate")
        .args(&["-s", "1M", from.to_str().unwrap()])
        .output()?;
    assert!(out.status.success());
    assert_eq!(from.metadata()?.len(), 1024 * 1024);

    let out = run(&[
        "--driver",
        drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ])?;
    assert!(out.status.success());
    assert_eq!(to.metadata()?.len(), 1024 * 1024);

    assert!(probably_sparse(&to)?);
    assert_eq!(quickstat(&from)?, quickstat(&to)?);

    let from_data = read(&from)?;
    let to_data = read(&to)?;
    assert_eq!(from_data, to_data);

    Ok(())
}
