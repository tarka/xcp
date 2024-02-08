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

use std::fs::{create_dir_all, set_permissions, write, File, Permissions};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
use cfg_if::cfg_if;
use test_case::test_case;


mod util;
use crate::util::*;

#[test]
fn basic_help() {
    let out = run(&["--help"]).unwrap();

    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Usage: xcp [OPTIONS] [PATHS]..."));
}

#[test]
fn no_args() {
    let out = run(&[]).unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Insufficient arguments"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn source_missing(drv: &str) {
    let out = run(&["--driver", drv, "/this/should/not/exist", "/dev/null"]).unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Source does not exist"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn source_missing_globbed(drv: &str) {
    let out = run(&[
        "--driver",
        drv,
        "-g",
        "/this/should/not/exist/*.txt",
        "/dev/null",
    ])
    .unwrap();

    assert!(!out.status.success());
    assert!(out.status.code().unwrap() == 1);

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("No source files found"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dest_file_exists(drv: &str) {
    let dir = tempdir_rel().unwrap();
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
    ])
    .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn source_same_as_dest(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("dest");
    create_dir_all(&dest).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-r",
        dest.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Cannot copy a directory into itself"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dest_file_in_dir_exists(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source_path).unwrap();
        File::create(dest_path).unwrap();
    }

    let out = run(&[
        "--driver",
        drv,
        "--no-clobber",
        source_path.to_str().unwrap(),
        dir.path().to_str().unwrap(),
    ])
    .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn multiple_files_to_a_file(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source1_path = dir.path().join("source1.txt");
    let source2_path = dir.path().join("source2.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        File::create(&source1_path).unwrap();
        File::create(&source2_path).unwrap();
        File::create(&dest_path).unwrap();
    }

    let out = run(&[
        "--driver", drv,
        source1_path.to_str().unwrap(),
        source2_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Multiple sources and destination is not a directory"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn directory_to_a_file(drv: &str) {
    let src_dir = tempdir_rel().unwrap();
    let source_file = src_dir.path().join("file.txt");

    let dir = tempdir_rel().unwrap();
    let dest_file = dir.path().join("dest.txt");

    {
        File::create(&source_file).unwrap();
        File::create(&dest_file).unwrap();
    }

    let out = run(&[
        "-r",
        "--driver", drv,
        src_dir.path().to_str().unwrap(),
        dest_file.to_str().unwrap(),
    ]).unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Cannot copy a directory to a file"));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dest_file_exists_overwrites(drv: &str) {
    let dir = tempdir_rel().unwrap();
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
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(files_match(&source_path, &dest_path));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn same_file_no_overwrite(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa").unwrap();
    }

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        source_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(! out.status.success());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dest_file_exists_noclobber(drv: &str) {
    let dir = tempdir_rel().unwrap();
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
    ])
    .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Destination file exists"));
    assert!(!files_match(&source_path, &dest_path));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_reflink_auto(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver", drv,
        "--reflink=auto",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    // Should always work, even on non-CoW FS
    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_reflink_never(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver", drv,
        "--reflink=never",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    // Should always work
    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_perms")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_perms", ignore = "No FS support")]
fn file_copy_perms(drv: &str) {
    cfg_if! {
        if #[cfg(feature = "test_no_xattr")] {
            let fs_supports_xattr = false;
        } else {
            let fs_supports_xattr = true;
        }
    }
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    if fs_supports_xattr {
        xattr::set(&source_path, "user.test", b"my test").unwrap();
    }

    let mut perms = source_path.metadata().unwrap().permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
    assert!(dest_path.metadata().unwrap()
        .permissions().readonly());

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if fs_supports_xattr {
        assert_eq!(
            xattr::get(&dest_path, "user.test").unwrap().unwrap(),
            b"my test"
        );
    }
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_perms")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_perms", ignore = "No FS support")]
fn file_copy_no_perms(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();
    let mut perms = source_path.metadata().unwrap().permissions();
    perms.set_readonly(true);
    set_permissions(&source_path, perms).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--no-perms",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
    assert!(!dest_path.metadata().unwrap()
        .permissions().readonly());
}


#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_timestamps(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    set_time_past(&source_path).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));

    let smeta = source_path.metadata().unwrap();
    let dmeta = dest_path.metadata().unwrap();
    assert!(timestamps_same(&smeta.modified().unwrap(), &dmeta.modified().unwrap()));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_no_timestamps(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();
    set_time_past(&source_path).unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--no-timestamps",
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));

    let smeta = source_path.metadata().unwrap();
    let dmeta = dest_path.metadata().unwrap();
    assert!(!timestamps_same(&smeta.modified().unwrap(), &dmeta.modified().unwrap()));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_rel(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_path, text).unwrap());
    assert!(files_match(&source_path, &dest_path));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn file_copy_multiple(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.path().join("file1.txt"), dir.path().join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "-vv",
        f1.to_str().unwrap(),
        f2.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_empty_dir(drv: &str) {
    let dir = tempdir_rel().unwrap();

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
    ])
    .unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir").exists());
    assert!(dest_base.join("mydir").is_dir());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_all_dirs(drv: &str) {
    let dir = tempdir_rel().unwrap();

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
    ])
    .unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_all_dirs_rel(drv: &str) {
    let dir = tempdir_rel().unwrap();

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
    ])
    .unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/two/three/").exists());
    assert!(dest_base.join("mydir/one/two/three/").is_dir());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_dirs_files(drv: &str) {
    let dir = tempdir_rel().unwrap();

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
    ])
    .unwrap();

    assert!(out.status.success());

    assert!(dest_base.join("mydir/one/one.txt").is_file());
    assert!(dest_base.join("mydir/one/two/two.txt").is_file());
    assert!(dest_base.join("mydir/one/two/three/three.txt").is_file());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(not(feature = "test_run_expensive"), ignore = "Stress test")]
fn copy_generated_tree(drv: &str) {
    // Spam some output to keep CI from timing-out (hopefully).
    println!("Generating file tree...");
    let src = gen_global_filetree(false).unwrap();

    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("target");

    println!("Running copy...");
    let out = run(&[
        "--driver", drv,
        "-r",
        src.to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.status.success());

    println!("Compare trees...");
    compare_trees(&src, &dest).unwrap();
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_dirs_overwrites(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver", drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

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
    ])
    .unwrap();

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

        let source_path = dir.path().join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path).unwrap();
        create_file(&source_file, "orig").unwrap();

        let dest_base = dir.path().join("dest");
        create_dir_all(&dest_base).unwrap();
        let dest_file = dest_base.join("mydir/file.txt");

        let out = run(&[
            "-r",
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ])
        .unwrap();

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "orig").unwrap());
    }

    // Second pass `no target directory`
    {
        let dir = tempdir_rel().unwrap();

        let source_path = dir.path().join("mydir");
        let source_file = source_path.join("file.txt");
        create_dir_all(&source_path).unwrap();
        create_file(&source_file, "new content").unwrap();

        let dest_base = dir.path().join("dest");
        create_dir_all(&dest_base).unwrap();
        let dest_file = dest_base.join("file.txt");

        let out = run(&[
            "-r",
            "-T", //-no-target-directory
            source_path.to_str().unwrap(),
            dest_base.to_str().unwrap(),
        ])
        .unwrap();

        assert!(out.status.success());
        assert!(file_contains(&dest_file, "new content").unwrap());
    }
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dir_copy_to_nonexistent_is_rename(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.path().join("dest");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig").unwrap());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dir_overwrite_with_noclobber(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

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
    ])
    .unwrap();

    assert!(!out.status.success());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_symlinks", ignore = "No FS support")]
fn dir_copy_containing_symlinks(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    let source_file = source_path.join("file.txt");
    let source_rlink = source_path.join("link.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();
    symlink("file.txt", source_rlink).unwrap();
    symlink("/etc/hosts", source_path.join("hosts")).unwrap();

    let dest_base = dir.path().join("dest");
    let dest_file = dest_base.join("file.txt");
    let dest_rlink = source_path.join("link.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(dest_rlink
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(dest_base
        .join("hosts")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dir_copy_with_hidden_dir(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir/.hidden");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.path().join("dest/.hidden");
    let dest_file = dest_base.join("file.txt");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest_file.exists());
    assert!(file_contains(&dest_file, "orig").unwrap());
    assert!(files_match(&source_file, &dest_file));
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn dir_with_gitignore(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    create_dir_all(&source_path).unwrap();

    let source_file = source_path.join("file.txt");
    create_file(&source_file, "file content").unwrap();

    let ignore_file = source_path.join(".gitignore");
    create_file(&ignore_file, "/.ignored\n").unwrap();

    let ignored_path = dir.path().join("mydir/.ignored");
    let ignored_file = ignored_path.join("file.txt");
    create_dir_all(&ignored_path).unwrap();
    create_file(&ignored_file, "ignored content").unwrap();

    let hidden_path = dir.path().join("mydir/.hidden");
    let hidden_file = hidden_path.join("file.txt");
    create_dir_all(&hidden_path).unwrap();
    create_file(&hidden_file, "hidden content").unwrap();

    let dest_base = dir.path().join("dest");

    let out = run(&[
        "--driver",
        drv,
        "-r",
        "--gitignore",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest_base.join("file.txt").exists());
    assert!(dest_base.join(".gitignore").exists());

    assert!(!dest_base.join(".ignored").exists());

    assert!(dest_base.join(".hidden").exists());
    assert!(dest_base.join(".hidden/file.txt").exists());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_with_glob(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.path().join("file1.txt"), dir.path().join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.path().join("file*.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest.join("file1.txt").exists());
    assert!(dest.join("file2.txt").exists());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_pattern_no_glob(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("dest");
    create_dir_all(&dest).unwrap();

    let f1 = dir.path().join("a [b] c.txt");
    create_file(&f1, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        dir.path().join("a [b] c.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(dest.join("a [b] c.txt").exists());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn glob_pattern_error(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let dest = dir.path().join("dest");
    create_dir_all(&dest).unwrap();

    let (f1, f2) = (dir.path().join("file1.txt"), dir.path().join("file2.txt"));
    create_file(&f1, "test").unwrap();
    create_file(&f2, "test").unwrap();

    let out = run(&[
        "--driver",
        drv,
        "--glob",
        dir.path().join("file***.txt").to_str().unwrap(),
        dest.to_str().unwrap(),
    ])
    .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("Pattern syntax error"));
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_sockets")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_sockets", ignore = "No FS support")]
fn test_socket_file(drv: &str) {

    let dir = tempdir_rel().unwrap();
    let from = dir.path().join("from.sock");
    let to = dir.path().join("to.sock");

    let _sock = UnixListener::bind(&from).unwrap();
    let ftype = from.metadata().unwrap().file_type();
    assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());

    let out = run(&[
        "--driver", drv,
        from.to_str().unwrap(),
        to.to_str().unwrap(),
    ]).unwrap();
    assert!(out.status.success());

    assert!(to.exists());
    let ftype = to.metadata().unwrap().file_type();
    assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_sockets")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_sockets", ignore = "No FS support")]
fn test_sockets_dir(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let src_dir = dir.path().join("fromdir");
    create_dir_all(&src_dir).unwrap();

    let from = src_dir.join("from.sock");
    let _sock = UnixListener::bind(&from).unwrap();
    let ftype = from.metadata().unwrap().file_type();
    assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());

    let to_dir = dir.path().join("todir");
    let to = to_dir.join("from.sock");

    let out = run(&[
        "--driver", drv,
        "-r",
        src_dir.to_str().unwrap(),
        to_dir.to_str().unwrap(),
    ]).unwrap();
    assert!(out.status.success());

    assert!(to.exists());
    let ftype = to.metadata().unwrap().file_type();
    assert!(!ftype.is_file() && !ftype.is_dir() && !ftype.is_symlink());
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_perms")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_perms", ignore = "No FS support")]
fn unreadable_file_error(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");
    let text = "This is a test file.";

    create_file(&source_path, text).unwrap();

    let perms = Permissions::from_mode(0);
    set_permissions(&source_path, perms).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(!out.status.success());
}

#[cfg_attr(all(feature = "parblock", not(feature = "test_no_perms")), test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
#[cfg_attr(feature = "test_no_perms", ignore = "No FS support")]
fn dest_file_exists_not_writable(drv: &str) {
    let dir = tempdir_rel().unwrap();
    let source_path = dir.path().join("source.txt");
    let dest_path = dir.path().join("dest.txt");

    {
        create_file(&source_path, "falskjdfa;lskdjfa").unwrap();
        File::create(&dest_path).unwrap();
    }
    set_permissions(&dest_path, Permissions::from_mode(0)).unwrap();

    let out = run(&[
        "--driver",
        drv,
        source_path.to_str().unwrap(),
        dest_path.to_str().unwrap(),
    ])
    .unwrap();

    assert!(!out.status.success());
}

#[cfg_attr(feature = "parblock", test_case("parblock"; "Test with parallel block driver"))]
#[test_case("parfile"; "Test with parallel file driver")]
fn copy_dirs_backup(drv: &str) {
    let dir = tempdir_rel().unwrap();

    let source_path = dir.path().join("mydir");
    let source_file = source_path.join("file.txt");
    create_dir_all(&source_path).unwrap();
    create_file(&source_file, "orig").unwrap();

    let dest_base = dir.path().join("dest");
    create_dir_all(&dest_base).unwrap();
    let dummy = dest_base.join("mydir/dummy_dir"); // Non-file member test
    create_dir_all(&dummy).unwrap();
    let dest_file = dest_base.join("mydir/file.txt");

    let mut out = run(&[
        "--driver", drv,
        "-r",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "orig").unwrap());

    // -- //

    out = run(&[
        "--driver", drv,
        "-r",
        "--backup=auto",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.status.success());

    let backup1 = dest_base.join("mydir/file.txt.~1~");
    assert!(!backup1.exists());

    write(&source_file, "new content").unwrap();
    assert!(file_contains(&source_file, "new content").unwrap());

    out = run(&[
        "--driver", drv,
        "-r",
        "--backup=numbered",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "new content").unwrap());
    assert!(backup1.exists());
    assert!(files_match(&source_file, &dest_file));

    // -- //
    let backup2 = dest_base.join("mydir/file.txt.~2~");
    assert!(!backup2.exists());

    write(&source_file, "new content 2").unwrap();
    assert!(file_contains(&source_file, "new content 2").unwrap());

    out = run(&[
        "--driver", drv,
        "-r",
        "--backup=auto",
        source_path.to_str().unwrap(),
        dest_base.to_str().unwrap(),
    ])
    .unwrap();

    assert!(out.status.success());
    assert!(file_contains(&dest_file, "new content 2").unwrap());
    assert!(backup1.exists());
    assert!(backup2.exists());
    assert!(files_match(&source_file, &dest_file));
}
