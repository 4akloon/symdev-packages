//! The `publish` binary as a user runs it, with every `PUBLISH_*` variable removed;
//! nothing here reaches a bucket.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const VARIABLES: [&str; 4] = [
    "PUBLISH_PRIVATE_URL",
    "PUBLISH_PUBLIC_URL",
    "PUBLISH_ACCESS_KEY_ID",
    "PUBLISH_SECRET_ACCESS_KEY",
];

fn publish(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_publish"));
    for name in VARIABLES {
        command.env_remove(name);
    }
    command.current_dir(cwd).args(args).output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// A tree, a recipe for it and a source archive, under one temporary directory.
fn workspace(recipe: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("tree/epoc32/include")).unwrap();
    fs::write(dir.path().join("tree/epoc32/include/e32std.h"), "header").unwrap();
    fs::write(dir.path().join("recipe.toml"), recipe).unwrap();
    fs::write(dir.path().join("src.tar.gz"), "source").unwrap();
    fs::create_dir(dir.path().join("out")).unwrap();
    dir
}

fn archives(dir: &Path) -> Vec<String> {
    let names = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name());
    names.map(|n| n.into_string().unwrap()).collect()
}

#[test]
fn the_usage_names_both_buckets_and_their_arguments() {
    let cwd = tempfile::tempdir().unwrap();
    let private = text(&publish(cwd.path(), &["private", "--help"]).stdout);
    for part in [
        "<id>",
        "--from <dir>",
        "--recipe <recipe.toml>",
        "--dry-run",
    ] {
        assert!(private.contains(part), "{part}: {private}");
    }
    let public = text(&publish(cwd.path(), &["public", "--help"]).stdout);
    for part in [
        "<id>",
        "--from <prefix>",
        "--source-code <tar.gz>",
        "--recipe <recipe.toml>",
    ] {
        assert!(public.contains(part), "{part}: {public}");
    }
}

#[test]
fn a_dry_run_needs_no_bucket_and_no_key() {
    let ws = workspace(
        "id = \"gcce;12.1.0\"\nlicense = \"GPL-3.0-or-later\"\nhost = \"x86_64-linux\"\n",
    );
    let root = ws.path();
    let from = root.join("tree");
    let recipe = root.join("recipe.toml");
    let source = root.join("src.tar.gz");
    let args = [
        "public",
        "gcce;12.1.0",
        "--from",
        from.to_str().unwrap(),
        "--source-code",
        source.to_str().unwrap(),
        "--recipe",
        recipe.to_str().unwrap(),
        "--dry-run",
    ];
    let output = publish(&root.join("out"), &args);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let index = text(&output.stdout);
    assert!(index.starts_with("schema = 1"), "{index}");
    let packed = archives(&root.join("out"));
    assert_eq!(packed.len(), 1);
    assert!(
        index.contains(&format!("url = \"gcce/12.1.0/{}\"", packed[0])),
        "{index}"
    );
}

#[test]
fn a_private_recipe_without_its_hash_prints_the_line_to_record() {
    let ws = workspace(
        "id = \"sdk;s60-3rd-fp2;1.1\"\nlicense = \"LicenseRef-Nokia-S60-SDK-EULA\"\n\
         host = \"any\"\ninclude = [\"epoc32/include\"]\n",
    );
    let root = ws.path();
    let (from, recipe) = (root.join("tree"), root.join("recipe.toml"));
    let args = [
        "private",
        "sdk;s60-3rd-fp2;1.1",
        "--from",
        from.to_str().unwrap(),
        "--recipe",
        recipe.to_str().unwrap(),
        "--dry-run",
    ];
    let output = publish(&root.join("out"), &args);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    let sha = archives(&root.join("out"))[0]
        .trim_end_matches(".tar.gz")
        .to_string();
    assert!(
        stderr.contains(&format!(
            "error: sdk;s60-3rd-fp2;1.1: the packed archive has sha256 {sha}"
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!("record `sha256 = \"{sha}\"`")),
        "{stderr}"
    );
}

#[test]
fn an_upload_without_the_bucket_url_fails_before_packing() {
    let ws = workspace("");
    let root = ws.path();
    let (from, recipe) = (root.join("tree"), root.join("recipe.toml"));
    let args = [
        "private",
        "sdk;s60-3rd-fp2;1.1",
        "--from",
        from.to_str().unwrap(),
        "--recipe",
        recipe.to_str().unwrap(),
    ];
    let output = publish(&root.join("out"), &args);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("error: PUBLISH_PRIVATE_URL is not set"),
        "{stderr}"
    );
    assert!(archives(&root.join("out")).is_empty());
}

#[test]
fn a_recipe_that_makes_two_packages_publishes_the_one_named() {
    let ws = workspace(
        "git = \"https://github.com/4akloon/symdev\"\ntag = \"v0.1.0\"\nbuild = \"build.sh\"\n\n\
         [[package]]\nid = \"symdev;0.1.0\"\nlicense = \"MIT\"\nhost = \"x86_64-linux\"\n\n\
         [[package]]\nid = \"rust-sdk;0.1.0\"\nlicense = \"MIT\"\nhost = \"any\"\n",
    );
    let root = ws.path();
    let (from, recipe) = (root.join("tree"), root.join("recipe.toml"));
    let source = root.join("src.tar.gz");
    let dry_run = |id: &str| {
        let args = [
            "public",
            id,
            "--from",
            from.to_str().unwrap(),
            "--source-code",
            source.to_str().unwrap(),
            "--recipe",
            recipe.to_str().unwrap(),
            "--dry-run",
        ];
        publish(&root.join("out"), &args)
    };
    let output = dry_run("rust-sdk;0.1.0");
    assert!(output.status.success(), "{}", text(&output.stderr));
    let index = text(&output.stdout);
    assert!(index.contains("id = \"rust-sdk;0.1.0\""), "{index}");
    assert!(index.contains("host = \"any\""), "{index}");
    assert!(!index.contains("symdev;0.1.0"), "{index}");
    let output = dry_run("symdev;0.2.0");
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("`symdev;0.1.0`, `rust-sdk;0.1.0`"),
        "{stderr}"
    );
}

/// `publish file <args…> --dry-run`-style runs on a one-line install.sh.
fn file(args: &[&str]) -> (Output, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("install.sh"), "#!/bin/sh\n").unwrap();
    let mut all = vec!["file", "install.sh"];
    all.extend_from_slice(args);
    (publish(dir.path(), &all), dir)
}

const HEADERS: [&str; 4] = [
    "--content-type",
    "text/plain; charset=utf-8",
    "--cache-control",
    "no-cache",
];

#[test]
fn the_file_usage_names_its_arguments_and_both_buckets() {
    let cwd = tempfile::tempdir().unwrap();
    let usage = text(&publish(cwd.path(), &["file", "--help"]).stdout);
    for part in [
        "<path>",
        "--to <key>",
        "--bucket <bucket>",
        "public",
        "private",
        "--content-type <type>",
        "--cache-control <value>",
        "--dry-run",
    ] {
        assert!(usage.contains(part), "{part}: {usage}");
    }
}

#[test]
fn a_file_dry_run_needs_no_bucket_and_no_key() {
    let mut args = vec!["--to", "install.sh", "--bucket", "public", "--dry-run"];
    args.extend(HEADERS);
    let (output, _dir) = file(&args);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("dry run: would upload install.sh (10 bytes"),
        "{stderr}"
    );
    assert!(stderr.contains("Cache-Control: no-cache"), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn a_file_is_never_uploaded_as_the_index() {
    let mut args = vec!["--to", "index.toml", "--bucket", "public", "--dry-run"];
    args.extend(HEADERS);
    let (output, _dir) = file(&args);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("error: key `index.toml` is the bucket's index"),
        "{stderr}"
    );
}

#[test]
fn a_file_upload_names_the_variable_of_its_bucket() {
    let mut args = vec!["--to", "install.sh", "--bucket", "private"];
    args.extend(HEADERS);
    let (output, _dir) = file(&args);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("error: PUBLISH_PRIVATE_URL is not set"),
        "{stderr}"
    );
}
