use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;
use predicates::prelude::*;

fn chimera(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("chimera").unwrap();
    cmd.env("CHIMERA_HOME", home.path());
    cmd
}

fn add_file(home: &TempDir, work: &TempDir, name: &str, body: &str) {
    let file = work.child(name);
    file.write_str(body).unwrap();
    chimera(home)
        .args(["add", file.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn add_then_list() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "#!/bin/sh\necho deploy\n");
    chimera(&home)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("bash/deploy.sh"));
}

#[test]
fn search_by_name_and_content() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    add_file(&home, &work, "notes.txt", "kubernetes manifests here\n");

    chimera(&home)
        .args(["search", "deploy"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bash/deploy.sh"));
    chimera(&home)
        .args(["search", "kubernetes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("notes.txt"));
}

#[test]
fn search_no_match() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home)
        .args(["search", "zzznotexist"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no matches"));
}

#[test]
fn search_glob_filters_by_extension() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    add_file(&home, &work, "main.rs", "fn main() {}\n");
    chimera(&home)
        .args(["search", "*.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rust/main.rs"))
        .stdout(predicate::str::contains("deploy.sh").not());
}

#[test]
fn rm_removes_entry() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home).args(["rm", "bash/deploy.sh"]).assert().success();
    chimera(&home)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("deploy.sh").not());
}

#[test]
fn mv_renames_entry() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home)
        .args(["mv", "bash/deploy.sh", "bash/release.sh"])
        .assert()
        .success();
    chimera(&home)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("bash/release.sh"));
}

#[test]
fn tag_then_search_by_tag() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home)
        .args(["tag", "bash/deploy.sh", "--add", "favorite"])
        .assert()
        .success();
    chimera(&home)
        .args(["search", "favorite"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bash/deploy.sh"));
}

#[test]
fn describe_then_search_by_description() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home)
        .args(["describe", "bash/deploy.sh", "pinnedphrase here"])
        .assert()
        .success();
    chimera(&home)
        .args(["search", "pinnedphrase"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bash/deploy.sh"));
}

#[test]
fn copy_into_directory() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let dest = TempDir::new().unwrap();
    add_file(&home, &work, "deploy.sh", "echo deploy\n");
    chimera(&home)
        .args(["copy", "bash/deploy.sh", "--to", dest.path().to_str().unwrap()])
        .assert()
        .success();
    dest.child("deploy.sh").assert(predicate::path::exists());
}

#[test]
fn init_writes_config() {
    let home = TempDir::new().unwrap();
    chimera(&home).arg("init").assert().success();
    home.child("config.toml").assert(predicate::str::contains("theme"));
}

#[test]
fn tui_requires_a_terminal() {
    let home = TempDir::new().unwrap();
    chimera(&home)
        .assert()
        .failure()
        .stderr(predicate::str::contains("interactive terminal"));
}
