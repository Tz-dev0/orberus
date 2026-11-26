//! End-to-end test: a fake `claude` prints what it receives, and the real
//! `cprofile` binary runs in front of it, both as a CLI and as the shim.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CPROFILE: &str = env!("CARGO_BIN_EXE_cprofile");

struct World {
    _temp: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    path: String,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let root = home.join(".claude-profiles");
        let real_bin = home.join("real-bin");
        fs::create_dir_all(&real_bin).unwrap();
        let fake = real_bin.join("claude");
        fs::write(
            &fake,
            "#!/bin/sh\necho \"dir=${CLAUDE_CONFIG_DIR:-unset}\"\necho \"session=${CLAUDE_PROFILES_SESSION:-unset}\"\necho \"args=$*\"\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!(
            "{}:{}:/usr/bin:/bin",
            root.join("bin").display(),
            real_bin.display()
        );
        Self {
            _temp: temp,
            home,
            root,
            path,
        }
    }

    fn command(&self, program: &Path, cwd: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(cwd)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.path);
        command
    }

    fn cprofile(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(Path::new(CPROFILE), cwd)
            .args(args)
            .output()
            .unwrap()
    }

    fn ok(&self, cwd: &Path, args: &[&str]) -> String {
        let output = self.cprofile(cwd, args);
        assert!(
            output.status.success(),
            "cprofile {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn shim(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = self.command(&self.root.join("bin/claude"), cwd);
        command.args(args);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().unwrap()
    }

    fn repo(&self, relative: &str, remote: &str) -> PathBuf {
        let repo = self.home.join(relative);
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(
            repo.join(".git/config"),
            format!("[remote \"origin\"]\n\turl = {remote}\n"),
        )
        .unwrap();
        repo
    }
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn without_config_the_shim_changes_nothing() {
    let world = World::new();
    // Install the link by hand: no config file is created.
    fs::create_dir_all(world.root.join("bin")).unwrap();
    symlink(CPROFILE, world.root.join("bin/claude")).unwrap();
    let output = world.shim(&world.home, &["--version"], &[]);
    assert!(output.status.success());
    assert_eq!(text(&output), "dir=unset\nsession=unset\nargs=--version\n");
}

#[test]
fn full_flow_picks_the_account_from_the_folder() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    world.ok(
        &home,
        &[
            "rule",
            "add",
            "--git-remote",
            "github.com/acme/*",
            "--profile",
            "work",
            "--locked",
        ],
    );

    let work_repo = world.repo("code/app", "git@github.com:acme/app.git");
    let own_repo = world.repo("code/mine", "git@github.com:me/mine.git");
    let work_dir = world.root.join("profiles/work");

    // Company repository: work account, arguments untouched.
    let output = world.shim(&work_repo, &["-p", "hello world"], &[]);
    assert!(output.status.success());
    assert_eq!(
        text(&output),
        format!(
            "dir={}\nsession=work\nargs=-p hello world\n",
            work_dir.display()
        )
    );

    // Personal repository: native profile, the variable stays unset even if
    // something had set it before.
    let output = world.shim(&own_repo, &[], &[("CLAUDE_CONFIG_DIR", "/somewhere/else")]);
    assert_eq!(text(&output), "dir=unset\nsession=perso\nargs=\n");

    // Guard: forcing the personal account in the company repository fails.
    let output = world.shim(&work_repo, &[], &[("CLAUDE_PROFILE", "perso")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).is_empty(), "claude must not start");
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked to profile 'work'"));

    // Explicit choice elsewhere is allowed.
    let output = world.shim(&own_repo, &[], &[("CLAUDE_PROFILE", "work")]);
    assert_eq!(
        text(&output),
        format!("dir={}\nsession=work\nargs=\n", work_dir.display())
    );

    // Inside a running session, the account is inherited, not resolved again.
    let output = world.shim(
        &work_repo,
        &[],
        &[
            ("CLAUDE_PROFILES_SESSION", "perso"),
            ("CLAUDE_CONFIG_DIR", "/kept"),
        ],
    );
    assert_eq!(text(&output), "dir=/kept\nsession=perso\nargs=\n");

    // `cprofile run` follows the same rules, and `use` pins a folder.
    let output = world.cprofile(&work_repo, &["run", "--", "--resume"]);
    assert_eq!(
        text(&output),
        format!("dir={}\nsession=work\nargs=--resume\n", work_dir.display())
    );
    world.ok(&own_repo, &["use", "work"]);
    let output = world.shim(&own_repo, &[], &[]);
    assert_eq!(
        text(&output),
        format!("dir={}\nsession=work\nargs=\n", work_dir.display())
    );

    let status = world.ok(&work_repo, &["status"]);
    assert!(status.contains("Profile  work"), "{status}");
    assert!(status.contains("locked rule"), "{status}");
    let doctor = world.cprofile(&home, &["doctor"]);
    assert!(doctor.status.success(), "{}", text(&doctor));
}

/// Runs `cprofile` and returns its error message, checking that it failed.
fn refused(world: &World, cwd: &Path, args: &[&str]) -> String {
    let output = world.cprofile(cwd, args);
    assert_eq!(output.status.code(), Some(2), "cprofile {args:?} must fail");
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn profiles_and_rules_can_be_listed_and_removed() {
    let world = World::new();
    let home = world.home.clone();

    // First contact: no argument gives a short guide, not an error.
    let guide = world.ok(&home, &[]);
    assert!(guide.contains("cprofile add"), "{guide}");

    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    world.ok(&home, &["add", "client"]);
    world.ok(
        &home,
        &["rule", "add", "--path", "~/clients", "--profile", "client"],
    );
    world.ok(
        &home,
        &[
            "rule",
            "add",
            "--git-remote",
            "github.com/acme/*",
            "-p",
            "work",
        ],
    );

    // The list shows who is logged in, read from Claude Code's state file.
    fs::write(
        world.root.join("profiles/work/.claude.json"),
        r#"{"oauthAccount": {"emailAddress": "me@acme.test"}}"#,
    )
    .unwrap();
    fs::write(world.root.join("profiles/client/.claude.json"), "{}").unwrap();
    let list = world.ok(&home, &["ls"]);
    assert!(list.contains("me@acme.test"), "{list}");
    assert_eq!(list.matches("not logged in").count(), 2, "{list}");

    // `disk` measures each profile and says where the space goes.
    fs::create_dir_all(world.root.join("profiles/work/projects/app")).unwrap();
    fs::write(
        world.root.join("profiles/work/projects/app/session.jsonl"),
        vec![b'x'; 300_000],
    )
    .unwrap();
    let disk = world.ok(&home, &["disk"]);
    assert!(disk.contains("Sessions") && disk.contains("work"), "{disk}");
    assert!(disk.contains("KB"), "{disk}");
    assert!(disk.contains("Nothing is deleted"), "{disk}");

    // Output is plain text when it is not a terminal: no color codes.
    let list = world.ok(&home, &["ls"]);
    assert!(list.contains("perso") && list.contains("client"), "{list}");
    assert!(!list.contains('\u{1b}'), "{list}");
    let rules = world.ok(&home, &["rule", "ls"]);
    assert!(rules.contains("folder ~/clients"), "{rules}");
    assert!(rules.contains("git remote github.com/acme/*"), "{rules}");

    // Nobody can answer a question here, so a missing argument is an error
    // that names what to type, and a removal needs --yes.
    assert!(refused(&world, &home, &["use"]).contains("missing profile name"));
    assert!(refused(&world, &home, &["use", "wrok"]).contains("available: client, perso, work"));
    assert!(refused(&world, &home, &["remove", "client"]).contains("--yes"));
    assert!(refused(&world, &home, &["rule", "rm", "1"]).contains("--yes"));
    assert!(refused(&world, &home, &["rule", "rm", "9", "-y"]).contains("no rule 9"));

    // Removing a profile takes its rules with it and keeps its folder.
    // (`--purge` has its own test.)
    let client_dir = world.root.join("profiles/client");
    world.ok(&home, &["remove", "client", "--yes"]);
    assert!(client_dir.is_dir(), "the profile folder must be kept");
    let rules = world.ok(&home, &["rule", "list"]);
    assert!(!rules.contains("clients"), "{rules}");
    assert!(rules.contains("github.com/acme/*"), "{rules}");

    world.ok(&home, &["rule", "remove", "1", "--yes"]);
    assert!(world.ok(&home, &["rule", "list"]).contains("No rule yet"));

    // Removing the default profile leaves no default behind.
    world.ok(&home, &["remove", "perso", "-y"]);
    let config = fs::read_to_string(world.root.join("config.toml")).unwrap();
    assert!(!config.contains("default"), "{config}");
    assert!(!config.contains("perso"), "{config}");
}

#[test]
fn purge_deletes_the_profile_folder_but_never_the_native_one() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    let native_dir = home.join(".claude");
    fs::create_dir_all(&native_dir).unwrap();
    let work_dir = world.root.join("profiles/work");
    fs::write(work_dir.join("history.jsonl"), "{}").unwrap();

    // It still needs a confirmation.
    assert!(refused(&world, &home, &["remove", "work", "--purge"]).contains("--yes"));
    assert!(work_dir.is_dir());

    let done = world.ok(&home, &["remove", "work", "--purge", "--yes"]);
    assert!(done.contains("deleted its Claude Code folder"), "{done}");
    assert!(!work_dir.exists(), "the profile folder must be deleted");
    assert!(!world.ok(&home, &["ls"]).contains("work"));

    // The native profile is the user's own ~/.claude: refused, nothing changes.
    let error = refused(&world, &home, &["remove", "perso", "--purge", "--yes"]);
    assert!(error.contains("never deleted"), "{error}");
    assert!(native_dir.is_dir());
    assert!(world.ok(&home, &["ls"]).contains("perso"));

    // A folder set by hand that holds more than the profile is refused too.
    world.ok(&home, &["add", "wide", "--dir", home.to_str().unwrap()]);
    let error = refused(&world, &home, &["remove", "wide", "--purge", "--yes"]);
    assert!(error.contains("refusing to delete"), "{error}");
    assert!(home.is_dir());
}

#[test]
fn a_team_rule_cannot_be_removed_and_beats_a_pinned_folder() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    fs::write(
        world.root.join("team.toml"),
        "[[rule]]\ngit_remote = \"github.com/acme/*\"\nprofile = \"work\"\nlocked = true\n",
    )
    .unwrap();
    world.ok(&home, &["rule", "add", "--path", "~/mine", "-p", "perso"]);

    // Rule 1 is the team's, rule 2 is the user's.
    assert!(refused(&world, &home, &["rule", "rm", "1", "-y"]).contains("team.toml"));
    world.ok(&home, &["rule", "rm", "2", "-y"]);

    // Pinning a locked repository to another profile works, but warns that
    // the locked rule still decides.
    let work_repo = world.repo("code/app", "git@github.com:acme/app.git");
    let pinned = world.ok(&work_repo, &["use", "perso"]);
    assert!(pinned.contains("still use 'work'"), "{pinned}");
}
