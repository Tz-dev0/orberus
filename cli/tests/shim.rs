//! End-to-end test: a fake `claude` prints what it receives, and the real
//! `orberus` binary runs in front of it, both as a CLI and as the shim.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ORBERUS: &str = env!("CARGO_BIN_EXE_orberus");

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
        let root = home.join(".orberus");
        let real_bin = home.join("real-bin");
        fs::create_dir_all(&real_bin).unwrap();
        let fake = real_bin.join("claude");
        fs::write(
            &fake,
            "#!/bin/sh\necho \"dir=${CLAUDE_CONFIG_DIR:-unset}\"\necho \"session=${ORBERUS_SESSION:-unset}\"\necho \"args=$*\"\n",
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
            // Never the real /Applications: `uninstall` looks for the app there.
            .env("ORBERUS_APPS", self.home.join("Applications"))
            .env("PATH", &self.path);
        command
    }

    fn orberus(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(Path::new(ORBERUS), cwd)
            .args(args)
            .output()
            .unwrap()
    }

    fn fake_codex(&self) {
        let binary = self.home.join("real-bin/codex");
        fs::write(
            &binary,
            "#!/bin/sh\n[ -z \"${CODEX_API_KEY:-}${CODEX_ACCESS_TOKEN:-}\" ] || exit 42\nprintf 'home=%s\nclaude_session=%s\n' \"${CODEX_HOME:-unset}\" \"${ORBERUS_SESSION:-unset}\"\nfor arg do printf 'arg=<%s>\n' \"$arg\"; done\n",
        )
        .unwrap();
        fs::set_permissions(binary, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn ok(&self, cwd: &Path, args: &[&str]) -> String {
        let output = self.orberus(cwd, args);
        assert!(
            output.status.success(),
            "orberus {args:?} failed: {}",
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

    /// Starts the wrapper the way an editor extension does: its own copy
    /// of claude first, then claude's arguments.
    fn wrapper(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = self.command(&self.root.join("ide/claude-wrapper"), cwd);
        command.args(args);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().unwrap()
    }

    /// The copy of claude an editor extension carries. It says so, to tell
    /// it apart from the one in PATH.
    fn bundled(&self) -> PathBuf {
        let folder = self
            .home
            .join(".vscode/extensions/anthropic.claude-code-1.0/bin");
        fs::create_dir_all(&folder).unwrap();
        let copy = folder.join("claude");
        fs::write(
            &copy,
            "#!/bin/sh\necho \"bundled dir=${CLAUDE_CONFIG_DIR:-unset} session=${ORBERUS_SESSION:-unset} args=$*\"\n",
        )
        .unwrap();
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o755)).unwrap();
        copy
    }

    /// An editor of the VS Code family, as it leaves its folders behind,
    /// with these settings. Returns its settings file.
    fn editor(&self, name: &str, settings: &str) -> PathBuf {
        let base = if cfg!(target_os = "macos") {
            "Library/Application Support"
        } else {
            ".config"
        };
        let user = self.home.join(base).join(name).join("User");
        fs::create_dir_all(user.join("globalStorage")).unwrap();
        let file = user.join("settings.json");
        fs::write(&file, settings).unwrap();
        file
    }

    /// The app of an editor, with the two names its `product.json` gives:
    /// its settings folder and, under the home folder, its data folder.
    fn app(&self, name: &str, settings_folder: &str, data_folder: &str) {
        let folder = self
            .home
            .join("Applications")
            .join(name)
            .join("Contents/Resources/app");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("product.json"),
            format!(
                "{{\"nameShort\": \"{settings_folder}\", \"dataFolderName\": \"{data_folder}\"}}"
            ),
        )
        .unwrap();
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
fn codex_run_uses_the_shared_rules_and_its_own_profile_directories() {
    let world = World::new();
    let home = &world.home;
    world.fake_codex();
    world.ok(home, &["add", "personal", "--native"]);
    world.ok(home, &["add", "work", "--dir", "~/work-claude"]);
    world.ok(
        home,
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
    let work_repo = world.repo("code/work", "git@github.com:acme/app.git");
    let work_codex = world.root.join("codex/work");
    let personal_codex = world.root.join("codex/personal");

    let output = world
        .command(Path::new(ORBERUS), &work_repo)
        .args([
            "run",
            "--tool",
            "codex",
            "--",
            "--profile",
            "fast",
            "hello world",
        ])
        .env("CODEX_HOME", home.join(".codex"))
        .env("CODEX_API_KEY", "ambient-key")
        .env("CODEX_ACCESS_TOKEN", "ambient-token")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        text(&output),
        format!(
            "home={}\nclaude_session=unset\narg=<--profile>\narg=<fast>\narg=<hello world>\n",
            work_codex.display()
        )
    );

    let output = world.ok(home, &["run", "--tool", "codex", "--", "login", "status"]);
    assert_eq!(
        output,
        format!(
            "home={}\nclaude_session=unset\narg=<login>\narg=<status>\n",
            personal_codex.display()
        )
    );
    assert_eq!(
        fs::metadata(&work_codex).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(personal_codex.is_dir());
    assert!(!home.join(".codex").exists());
    assert!(!world.root.join("bin/codex").exists());

    let output = world.orberus(
        &work_repo,
        &[
            "run",
            "--tool",
            "codex",
            "--profile",
            "personal",
            "--",
            "login",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).is_empty(), "codex must not start");
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked to profile 'work'"));

    let output = world.ok(&work_repo, &["run", "--", "--resume"]);
    assert_eq!(
        output,
        format!(
            "dir={}\nsession=work\nargs=--resume\n",
            home.join("work-claude").display()
        )
    );
}

#[test]
fn codex_run_without_a_profile_keeps_the_existing_codex_home() {
    let world = World::new();
    world.fake_codex();
    let existing = world.home.join("existing-codex");
    let output = world
        .command(Path::new(ORBERUS), &world.home)
        .args(["run", "--tool", "codex", "--", "--version"])
        .env("CODEX_HOME", &existing)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        text(&output),
        format!(
            "home={}\nclaude_session=unset\narg=<--version>\n",
            existing.display()
        )
    );
    assert!(!world.root.join("codex").exists());
}

#[test]
fn codex_shim_is_opt_in_and_uses_the_requested_directory() {
    let world = World::new();
    let home = &world.home;
    world.fake_codex();
    world.ok(home, &["setup"]);
    let shim = world.root.join("bin/codex");
    assert!(!shim.exists());
    world.ok(home, &["setup", "--tool", "codex"]);
    assert!(shim.is_symlink());
    world.ok(home, &["add", "personal"]);
    world.ok(home, &["add", "work"]);
    assert!(world
        .ok(home, &["doctor", "--tool", "codex"])
        .contains("real Codex CLI is found"));
    let work = world.repo("code/work project", "git@github.com:acme/project.git");
    world.ok(
        home,
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

    for args in [
        vec!["-C", "code/work project"],
        vec!["exec", "--cd=code/work project", "--", "-C elsewhere"],
    ] {
        let output = world
            .command(&shim, home)
            .args(&args)
            .env("ORBERUS_SESSION", "personal")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(text(&output).starts_with(&format!(
            "home={}\n",
            world.root.join("codex/work").display()
        )));
        for arg in args {
            assert!(text(&output).contains(&format!("arg=<{arg}>\n")));
        }
    }
    let locked = world
        .command(&shim, home)
        .args(["--cd", work.to_str().unwrap()])
        .env("ORBERUS_PROFILE", "personal")
        .output()
        .unwrap();
    assert_eq!(locked.status.code(), Some(2));
    assert!(text(&locked).is_empty());
    assert!(String::from_utf8_lossy(&locked.stderr).contains("locked to profile 'work'"));

    world.ok(home, &["pause"]);
    let output = world
        .command(&shim, &work)
        .env("CODEX_HOME", "/inherited")
        .output()
        .unwrap();
    assert!(text(&output).starts_with("home=/inherited\n"));
    world.ok(home, &["resume"]);
    world.ok(home, &["uninstall", "--yes"]);
    assert!(!shim.is_symlink());
    assert!(world.home.join("real-bin/codex").is_file());
}

#[test]
fn setup_and_uninstall_leave_an_unrelated_codex_in_the_shim_directory() {
    let world = World::new();
    world.fake_codex();
    let shim = world.root.join("bin/codex");
    fs::create_dir_all(world.root.join("bin")).unwrap();
    fs::write(&shim, "not an Orberus shim").unwrap();
    assert!(
        refused(&world, &world.home, &["setup", "--tool", "codex"]).contains("refusing to replace")
    );
    world.ok(&world.home, &["uninstall", "--yes"]);
    assert_eq!(fs::read_to_string(shim).unwrap(), "not an Orberus shim");
}

#[test]
fn codex_run_uses_the_configured_binary_and_keeps_case_variants_separate() {
    let world = World::new();
    world.fake_codex();
    let binary = world.home.join("real-bin/codex-custom");
    fs::rename(world.home.join("real-bin/codex"), &binary).unwrap();
    fs::create_dir_all(&world.root).unwrap();
    fs::write(
        world.root.join("config.toml"),
        "codex_bin = '~/real-bin/codex-custom'\n[profiles.work]\n[profiles.Work]\n",
    )
    .unwrap();

    for (name, folder) in [("work", "work"), ("Work", "~work")] {
        let output = world
            .command(Path::new(ORBERUS), &world.home)
            .args(["run", "--tool", "codex", "-p", name])
            .env("ORBERUS_HOME", ".orberus")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let expected = world.root.join("codex").join(folder);
        assert!(expected.is_dir());
        assert_eq!(
            text(&output),
            format!("home={}\nclaude_session=unset\n", expected.display())
        );
    }
}

#[test]
fn without_config_the_shim_changes_nothing() {
    let world = World::new();
    // Install the link by hand: no config file is created.
    fs::create_dir_all(world.root.join("bin")).unwrap();
    symlink(ORBERUS, world.root.join("bin/claude")).unwrap();
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
    let output = world.shim(&work_repo, &[], &[("ORBERUS_PROFILE", "perso")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).is_empty(), "claude must not start");
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked to profile 'work'"));

    // Explicit choice elsewhere is allowed.
    let output = world.shim(&own_repo, &[], &[("ORBERUS_PROFILE", "work")]);
    assert_eq!(
        text(&output),
        format!("dir={}\nsession=work\nargs=\n", work_dir.display())
    );

    // Inside a running session, the account is inherited, not resolved again.
    let output = world.shim(
        &work_repo,
        &[],
        &[("ORBERUS_SESSION", "perso"), ("CLAUDE_CONFIG_DIR", "/kept")],
    );
    assert_eq!(text(&output), "dir=/kept\nsession=perso\nargs=\n");

    // `orberus run` follows the same rules, and `use` pins a folder.
    let output = world.orberus(&work_repo, &["run", "--", "--resume"]);
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
    let doctor = world.orberus(&home, &["doctor"]);
    assert!(doctor.status.success(), "{}", text(&doctor));
}

/// Runs `orberus` and returns its error message, checking that it failed.
fn refused(world: &World, cwd: &Path, args: &[&str]) -> String {
    let output = world.orberus(cwd, args);
    assert_eq!(output.status.code(), Some(2), "orberus {args:?} must fail");
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn profiles_and_rules_can_be_listed_and_removed() {
    let world = World::new();
    let home = world.home.clone();

    // First contact: no argument gives a short guide, not an error.
    let guide = world.ok(&home, &[]);
    assert!(guide.contains("orberus add"), "{guide}");

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

    // `retention` shows the 30-day default, sets a value in the profile's
    // own settings file, and keeps what was already there.
    let kept = world.ok(&home, &["retention"]);
    assert_eq!(kept.matches("Claude Code default").count(), 3, "{kept}");
    fs::write(
        world.root.join("profiles/work/settings.json"),
        r#"{"theme": "dark"}"#,
    )
    .unwrap();
    world.ok(&home, &["retention", "work", "120"]);
    let settings = fs::read_to_string(world.root.join("profiles/work/settings.json")).unwrap();
    assert!(settings.contains(r#""theme": "dark""#), "{settings}");
    assert!(
        settings.contains(r#""cleanupPeriodDays": 120"#),
        "{settings}"
    );
    let kept = world.ok(&home, &["retention"]);
    assert!(kept.contains("120 days"), "{kept}");
    assert!(refused(&world, &home, &["retention", "work"]).contains("missing number of days"));
    assert!(refused(&world, &home, &["retention", "work", "0"]).contains("1 day"));
    // It can be chosen when the profile is created.
    world.ok(&home, &["add", "short", "--keep-days", "365"]);
    assert!(world.ok(&home, &["retention"]).contains("1 year"));
    world.ok(&home, &["remove", "short", "--yes"]);

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
fn purge_signs_out_codex_before_deleting_its_home() {
    let world = World::new();
    let home = &world.home;
    let codex = home.join("real-bin/codex");
    fs::write(
        &codex,
        r#"#!/bin/sh
set -eu
entry="$HOME/keyring-${CODEX_HOME##*/}"
case "$*" in
    login) touch "$entry" "$CODEX_HOME/auth.json" ;;
    'login status') test -f "$entry" ;;
    logout)
        test -f "$CODEX_HOME/auth.json"
        test ! -f "$CODEX_HOME/fail-logout"
        rm "$entry"
        ;;
    *) exit 2 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(codex, fs::Permissions::from_mode(0o755)).unwrap();

    for profile in ["work", "personal"] {
        world.ok(home, &["add", profile]);
        world.ok(
            home,
            &["run", "--tool", "codex", "-p", profile, "--", "login"],
        );
    }
    let work_codex = world.root.join("codex/work");
    let fail_logout = work_codex.join("fail-logout");
    fs::write(&fail_logout, "").unwrap();
    let args = ["remove", "work", "--purge", "--yes"];
    assert!(refused(&world, home, &args).contains("Codex"));
    assert!(world.root.join("profiles/work").is_dir());
    assert!(work_codex.join("auth.json").is_file());
    assert!(world.ok(home, &["list"]).contains("work"));

    fs::remove_file(fail_logout).unwrap();
    world.ok(home, &args);
    assert!(!work_codex.exists());
    world.ok(home, &["add", "work"]);
    let login_status = |profile| {
        world.orberus(
            home,
            &[
                "run", "--tool", "codex", "-p", profile, "--", "login", "status",
            ],
        )
    };
    assert_eq!(login_status("work").status.code(), Some(1));
    assert!(login_status("personal").status.success());
}

// The Trash is only known on macOS for now.
#[cfg(target_os = "macos")]
#[test]
fn clean_lists_conversations_and_moves_the_chosen_one_to_the_trash() {
    use std::time::{Duration, SystemTime};

    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["add", "work"]);
    assert!(world
        .ok(&home, &["clean", "work"])
        .contains("no conversation"));

    let project = world.root.join("profiles/work/projects/-Users-me-app");
    fs::create_dir_all(project.join("memory")).unwrap();
    fs::write(project.join("memory/MEMORY.md"), "- a note").unwrap();
    let old = "11111111-2222-3333-4444-555555555555";
    let fresh = "99999999-2222-3333-4444-555555555555";
    for id in [old, fresh] {
        fs::write(
            project.join(format!("{id}.jsonl")),
            "{\"type\":\"custom-title\",\"customTitle\":\"Fix the login\"}\n",
        )
        .unwrap();
    }
    // Last used three days ago. The other one was written just now.
    let three_days_ago = SystemTime::now() - Duration::from_secs(3 * 86_400);
    fs::File::options()
        .write(true)
        .open(project.join(format!("{old}.jsonl")))
        .unwrap()
        .set_modified(three_days_ago)
        .unwrap();

    // Without a choice it only lists.
    let list = world.ok(&home, &["clean", "work"]);
    assert!(list.contains("Fix the login"), "{list}");
    assert!(
        list.contains("3 days ago") && list.contains("in use?"),
        "{list}"
    );
    assert!(list.contains("Nothing was changed"), "{list}");
    assert!(project.join(format!("{old}.jsonl")).exists());

    // A choice still needs a confirmation, and an unknown id is an error.
    assert!(refused(&world, &home, &["clean", "work", "--session", "1111"]).contains("--yes"));
    assert!(
        refused(&world, &home, &["clean", "work", "--session", "abcd", "-y"])
            .contains("no conversation")
    );

    fs::create_dir_all(home.join(".Trash")).unwrap();
    let done = world.ok(&home, &["clean", "work", "--session", "1111", "--yes"]);
    assert!(done.contains("moved to the Trash"), "{done}");
    assert!(!project.join(format!("{old}.jsonl")).exists());
    let moved = home
        .join(".Trash")
        .join(format!("Claude conversation {old}"));
    assert!(moved.join(format!("{old}.jsonl")).is_file());
    assert!(moved.join("where it came from.txt").is_file());
    // Memory and the other conversation are untouched.
    assert!(project.join("memory/MEMORY.md").is_file());
    assert!(project.join(format!("{fresh}.jsonl")).is_file());

    // One used in the last hour is refused: it may still be open.
    let output = world.orberus(&home, &["clean", "work", "--session", "9999", "--yes"]);
    assert_eq!(output.status.code(), Some(1));
    // Each conversation reports on its own line, on standard output.
    assert!(String::from_utf8_lossy(&output.stdout).contains("last hour"));
    assert!(project.join(format!("{fresh}.jsonl")).is_file());
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

#[test]
fn uninstall_removes_the_shim_and_the_path_line_but_keeps_the_profiles() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    let shim = world.root.join("bin/claude");
    assert!(shim.symlink_metadata().is_ok());

    // What an interactive `setup` writes, around lines of the user's own.
    let zshrc = home.join(".zshrc");
    fs::write(
        &zshrc,
        "export EDITOR=vim\n\n# orberus: the claude shim must come before the real claude\nexport PATH=\"$HOME/.orberus/bin:$PATH\"\nalias ll='ls -l'\n",
    )
    .unwrap();
    let native_dir = home.join(".claude");
    fs::create_dir_all(&native_dir).unwrap();

    // It needs a confirmation, and nothing moves without it.
    assert!(refused(&world, &home, &["uninstall"]).contains("--yes"));
    assert!(shim.symlink_metadata().is_ok());

    let done = world.ok(&home, &["uninstall", "--yes"]);
    assert!(done.contains("Shim removed"), "{done}");
    assert!(shim.symlink_metadata().is_err(), "the shim must be gone");
    assert_eq!(
        fs::read_to_string(&zshrc).unwrap(),
        "export EDITOR=vim\nalias ll='ls -l'\n"
    );
    // The test binary lives in cargo's folder, not in ours: it is not removed.
    assert!(done.contains("orberus itself is still at"), "{done}");

    // Profiles, their folders and ~/.claude are all still there.
    assert!(world.root.join("config.toml").is_file());
    assert!(world.root.join("profiles/work").is_dir());
    assert!(native_dir.is_dir());
    assert!(world.ok(&home, &["ls"]).contains("work"));
}

#[test]
fn each_profile_has_its_claude_desktop_data_and_a_purge_deletes_it() {
    let world = World::new();
    let home = world.home.clone();
    assert!(world.ok(&home, &["desktop"]).contains("No profile yet"));
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);

    // This world's Applications folder has no Claude Desktop: the profiles
    // are listed all the same, and opening one says why it cannot.
    let listed = world.ok(&home, &["desktop"]);
    assert!(
        listed.contains("~/Library/Application Support/Claude (Claude Desktop's own)"),
        "{listed}"
    );
    assert!(listed.contains("~/.orberus/desktop/work"), "{listed}");
    assert!(listed.contains("not opened yet"), "{listed}");
    let error = refused(&world, &home, &["desktop", "work"]);
    assert!(error.contains("Claude Desktop was not found"), "{error}");
    // Nothing runs, so nothing would close: `--yes` changes nothing here.
    let error = refused(&world, &home, &["desktop", "work", "--yes"]);
    assert!(error.contains("Claude Desktop was not found"), "{error}");
    let error = refused(&world, &home, &["desktop", "nobody"]);
    assert!(error.contains("unknown profile"), "{error}");
    assert!(!world.root.join("desktop").exists(), "nothing is created");

    // As after a first opening: each copy has its data folder.
    let data = world.root.join("desktop/work");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("Cookies"), "").unwrap();
    let usual = home.join("Library/Application Support/Claude");
    fs::create_dir_all(&usual).unwrap();
    assert!(world.ok(&home, &["desktop"]).contains("closed"));

    // Removing the profile keeps that data, as it keeps its Claude Code
    // folder: a profile of the same name finds both again.
    world.ok(&home, &["remove", "work", "--yes"]);
    assert!(data.join("Cookies").is_file());
    world.ok(&home, &["add", "work"]);

    let done = world.ok(&home, &["remove", "work", "--purge", "--yes"]);
    assert!(
        done.contains("deleted the data of its Claude Desktop"),
        "{done}"
    );
    assert!(!data.exists(), "the copy's data must be deleted");
    assert!(usual.is_dir(), "Claude Desktop's own data is never ours");
}

#[test]
fn uninstall_purge_deletes_the_profiles_but_never_the_native_folder() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    world.ok(&home, &["add", "perso", "--native"]);
    world.ok(&home, &["add", "work"]);
    let native_dir = home.join(".claude");
    fs::create_dir_all(&native_dir).unwrap();
    fs::write(native_dir.join("history.jsonl"), "{}").unwrap();
    // A profile folder set by hand, outside the data folder.
    let custom = home.join("elsewhere/client");
    world.ok(&home, &["add", "client", "--dir", custom.to_str().unwrap()]);
    fs::create_dir_all(&custom).unwrap();
    // And one that holds far more than a profile: it must survive.
    world.ok(&home, &["add", "wide", "--dir", home.to_str().unwrap()]);

    let done = world.ok(&home, &["uninstall", "--purge", "--yes"]);
    assert!(done.contains("refusing to delete"), "{done}");
    assert!(!world.root.exists(), "the data folder must be deleted");
    assert!(!custom.exists(), "a profile folder set by hand goes too");
    assert!(home.is_dir());
    assert!(native_dir.join("history.jsonl").is_file());
}

// The app only exists on macOS.
#[cfg(target_os = "macos")]
#[test]
fn uninstall_moves_the_app_to_the_trash_only_when_it_is_ours() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    let app = home.join("Applications/Orberus.app");
    fs::create_dir_all(app.join("Contents")).unwrap();

    // Same name, another identifier: not our app.
    fs::write(
        app.join("Contents/Info.plist"),
        "<string>com.example.other</string>",
    )
    .unwrap();
    world.ok(&home, &["uninstall", "--yes"]);
    assert!(app.is_dir(), "an app that is not ours must stay");

    fs::write(
        app.join("Contents/Info.plist"),
        "<string>com.github.tzdev0.orberus</string>",
    )
    .unwrap();
    let done = world.ok(&home, &["uninstall", "--yes"]);
    assert!(done.contains("moved to the Trash"), "{done}");
    assert!(!app.exists());
    assert!(home.join(".Trash/Orberus.app").is_dir());
}

// A copy kept outside the Applications folders (a build folder, Downloads)
// is found through the file that opens it at login.
#[cfg(target_os = "macos")]
#[test]
fn uninstall_finds_the_app_outside_the_applications_folders() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    let plist = "<string>com.github.tzdev0.orberus</string>";
    let usual = home.join("Applications/Orberus.app");
    let built = home.join("builds/macos/Orberus.app");
    for app in [&usual, &built] {
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::write(app.join("Contents/Info.plist"), plist).unwrap();
    }
    // What the app writes when it starts: a file that names its program.
    let agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&agents).unwrap();
    let login = agents.join("com.github.tzdev0.orberus.plist");
    let names = |app: &Path| {
        format!(
            "<dict><key>ProgramArguments</key><array><string>{}</string></array></dict>",
            app.join("Contents/MacOS/orberus-app").display()
        )
    };

    // The file names a folder that is not our app: it stays.
    let other = home.join("builds/Other.app");
    fs::create_dir_all(other.join("Contents/MacOS")).unwrap();
    fs::write(
        other.join("Contents/Info.plist"),
        "<string>com.example.other</string>",
    )
    .unwrap();
    fs::write(&login, names(&other)).unwrap();
    fs::remove_dir_all(&usual).unwrap();
    world.ok(&home, &["uninstall", "--yes"]);
    assert!(other.is_dir(), "an app that is not ours must stay");
    assert!(
        built.is_dir(),
        "a copy nothing points to is not searched for"
    );

    // The file names our copy in the build folder: both copies go.
    fs::create_dir_all(usual.join("Contents")).unwrap();
    fs::write(usual.join("Contents/Info.plist"), plist).unwrap();
    fs::write(&login, names(&built)).unwrap();
    let done = world.ok(&home, &["uninstall", "--yes"]);
    assert!(
        done.contains("the menu bar app, moved to the Trash: ~/builds/macos/Orberus.app"),
        "{done}"
    );
    assert!(!usual.exists());
    assert!(!built.exists());
    assert!(home.join(".Trash/Orberus.app").is_dir());
    assert!(home.join(".Trash/Orberus 2.app").is_dir());
    assert!(!login.exists());
}

#[test]
fn usage_shows_each_folder_against_the_rules() {
    let world = World::new();
    let home = world.home.clone();
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
        ],
    );
    assert!(world
        .ok(&home, &["usage", "work"])
        .contains("No conversation in this period"));

    // Four folders were used with the work account: a company repository,
    // a side project no rule names, a folder pinned to the other profile
    // since, and one that was deleted.
    let acme = world.repo("code/acme-api", "git@github.com:acme/api.git");
    let blog = world.repo("code/blog", "git@github.com:me/blog.git");
    let pinned = home.join("code/notes");
    fs::create_dir_all(&pinned).unwrap();
    world.ok(&pinned, &["use", "perso"]);
    let gone = home.join("code/deleted");

    let answer = |id: &str, folder: &Path, output: u64| {
        format!(
            "{{\"type\":\"assistant\",\"cwd\":\"{}\",\"timestamp\":\"2026-11-26T10:00:00.000Z\",\"requestId\":\"r-{id}\",\"message\":{{\"id\":\"{id}\",\"usage\":{{\"input_tokens\":0,\"output_tokens\":{output},\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}}}}}\n",
            folder.display()
        )
    };
    let project = world.root.join("profiles/work/projects/-p");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("11111111-2222-3333-4444-555555555555.jsonl"),
        [
            answer("a", &acme, 600),
            // Written twice, as Claude Code does: counted once.
            answer("a", &acme, 600),
            answer("b", &blog, 300),
            answer("c", &pinned, 60),
            answer("d", &gone, 40),
        ]
        .concat(),
    )
    .unwrap();

    // A period long enough to hold the date written above, whenever the
    // test runs.
    let out = world.ok(&home, &["usage", "work", "--days", "36500"]);
    let row = |folder: &str| {
        out.lines()
            .find(|line| line.contains(folder))
            .unwrap_or_else(|| panic!("no row for {folder} in:\n{out}"))
            .to_string()
    };
    assert!(row("~/code/acme-api").contains("60%"), "{out}");
    assert!(
        row("~/code/acme-api").contains("✓ rule git_remote"),
        "{out}"
    );
    // No rule names the blog: started there today, `claude` would use the
    // default profile, not this one.
    assert!(row("~/code/blog").contains("30%"), "{out}");
    assert!(
        row("~/code/blog").contains("would use perso today (default profile)"),
        "{out}"
    );
    assert!(
        row("~/code/notes").contains("would use perso today (pinned with .orberus-profile)"),
        "{out}"
    );
    assert!(row("~/code/deleted").contains("folder gone"), "{out}");
    assert!(
        out.contains("36% came from 2 project(s) where `claude` would use another profile today"),
        "{out}"
    );
    assert!(out.contains("cd ~/code/blog && orberus use work"), "{out}");

    // Nothing was used with the other profile yet.
    let all = world.ok(&home, &["usage", "--days", "36500"]);
    assert!(all.contains("No conversation in this period"), "{all}");

    // The same blog, used with the default profile: nothing is wrong, but no
    // rule says so.
    let project = home.join(".claude/projects/-p");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("22222222-2222-3333-4444-555555555555.jsonl"),
        [answer("e", &blog, 10), answer("f", &pinned, 10)].concat(),
    )
    .unwrap();
    let out = world.ok(&home, &["usage", "perso", "--days", "36500"]);
    assert!(out.contains("no rule, default profile"), "{out}");
    assert!(out.contains("✓ pinned with .orberus-profile"), "{out}");
    assert!(
        out.contains("50% came from 1 project(s) no rule names"),
        "{out}"
    );
    assert!(out.contains("cd ~/code/blog && orberus rule add"), "{out}");

    assert!(world.orberus(&home, &["usage", "nope"]).status.code() != Some(0));
}

#[test]
fn usage_lists_conversations_and_flags_the_ones_that_carry_a_lot() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["add", "work"]);
    let api = world.repo("code/acme-api", "git@github.com:acme/api.git");

    // `read` is what the conversation sent again with that answer.
    let answer = |id: &str, read: u64| {
        format!(
            "{{\"type\":\"assistant\",\"cwd\":\"{}\",\"timestamp\":\"2026-11-26T10:00:00.000Z\",\"requestId\":\"r-{id}\",\"message\":{{\"id\":\"{id}\",\"usage\":{{\"input_tokens\":0,\"output_tokens\":1000,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":{read}}}}}}}\n",
            api.display()
        )
    };
    let project = world.root.join("profiles/work/projects/-p");
    fs::create_dir_all(&project).unwrap();
    let long = "aaaaaaaa-2222-3333-4444-555555555555";
    let short = "bbbbbbbb-2222-3333-4444-555555555555";
    fs::write(
        project.join(format!("{long}.jsonl")),
        [
            "{\"type\":\"custom-title\",\"customTitle\":\"Payment retries\"}\n".to_string(),
            answer("a", 400_000),
            answer("b", 500_000),
        ]
        .concat(),
    )
    .unwrap();
    fs::write(project.join(format!("{short}.jsonl")), answer("c", 30_000)).unwrap();

    let out = world.ok(
        &home,
        &["usage", "work", "--conversations", "--days", "36500"],
    );
    let row = |id: &str| {
        out.lines()
            .find(|line| line.contains(id))
            .unwrap_or_else(|| panic!("no row for {id} in:\n{out}"))
            .to_string()
    };
    // The average of what its two answers sent.
    assert!(row("aaaaaaaa").contains("450k"), "{out}");
    assert!(row("aaaaaaaa").contains("Payment retries"), "{out}");
    assert!(row("aaaaaaaa").contains("~/code/acme-api"), "{out}");
    assert!(row("bbbbbbbb").contains("30k"), "{out}");
    assert!(row("bbbbbbbb").contains("(untitled)"), "{out}");
    assert!(
        out.contains("1 conversation(s) send more than 200k tokens with each answer"),
        "{out}"
    );
    assert!(out.contains("/compact"), "{out}");
}

#[test]
fn the_wrapper_starts_the_extensions_own_copy_with_the_account_of_the_folder() {
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
    let copy = world.bundled();
    let copy = copy.to_str().unwrap();

    // The extension's copy runs, not the one in PATH; its path is taken
    // out of the arguments, the others are untouched.
    let output = world.wrapper(&work_repo, &[copy, "auth", "status", "--json"], &[]);
    assert!(output.status.success());
    assert_eq!(
        text(&output),
        format!(
            "bundled dir={} session=work args=auth status --json\n",
            work_dir.display()
        )
    );

    // Native profile: the variable is unset, whatever the editor had.
    let output = world.wrapper(
        &own_repo,
        &[copy, "--print"],
        &[("CLAUDE_CONFIG_DIR", "/somewhere/else")],
    );
    assert_eq!(
        text(&output),
        "bundled dir=unset session=perso args=--print\n"
    );

    // An editor opened from a session's terminal carries that session's
    // variables. Its windows still get the account of their own folder.
    let output = world.wrapper(
        &work_repo,
        &[copy],
        &[("ORBERUS_SESSION", "perso"), ("CLAUDE_CONFIG_DIR", "/kept")],
    );
    assert_eq!(
        text(&output),
        format!("bundled dir={} session=work args=\n", work_dir.display())
    );

    // The guard holds in an editor too, and its message reaches stderr,
    // which the extension shows.
    let output = world.wrapper(&work_repo, &[copy], &[("ORBERUS_PROFILE", "perso")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).is_empty(), "claude must not start");
    let message = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(message.contains("locked to profile 'work'"), "{message}");

    // An extension with no copy of its own passes only arguments: the
    // real claude of PATH is started, never the wrapper or the shim again.
    let output = world.wrapper(&work_repo, &["--print", "hello"], &[]);
    assert_eq!(
        text(&output),
        format!(
            "dir={}\nsession=work\nargs=--print hello\n",
            work_dir.display()
        )
    );

    // A copy that is not there is an error, not a prompt for claude.
    let output = world.wrapper(&work_repo, &["/nowhere/claude", "--print"], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).is_empty());
}

#[test]
fn without_config_the_wrapper_changes_nothing() {
    let world = World::new();
    fs::create_dir_all(world.root.join("ide")).unwrap();
    symlink(ORBERUS, world.root.join("ide/claude-wrapper")).unwrap();
    let copy = world.bundled();
    let output = world.wrapper(
        &world.home,
        &[copy.to_str().unwrap(), "--version"],
        &[("CLAUDE_CONFIG_DIR", "/set/by/the/editor")],
    );
    assert!(output.status.success());
    assert_eq!(
        text(&output),
        "bundled dir=/set/by/the/editor session=unset args=--version\n"
    );
}

#[test]
fn ide_sends_the_editors_through_the_wrapper_and_back() {
    let world = World::new();
    let home = world.home.clone();
    let wrapper = world.root.join("ide/claude-wrapper");
    let before = "{\n    // my settings\n    \"editor.fontSize\": 13,\n}\n";
    let code = world.editor("Code", before);
    let cursor = world.editor("Cursor", "{}");
    let key = "claudeCode.claudeProcessWrapper";

    // Nothing is on yet, and the command says how to change that.
    let listed = world.ok(&home, &["ide"]);
    assert!(listed.contains("VS Code"), "{listed}");
    assert!(listed.contains("Cursor"), "{listed}");
    assert!(listed.contains("skips Orberus"), "{listed}");
    assert!(listed.contains("orberus ide on"), "{listed}");

    // `on` works before `setup`: it installs the wrapper it points to.
    let done = world.ok(&home, &["ide", "on"]);
    assert!(done.contains("VS Code"), "{done}");
    assert!(wrapper.symlink_metadata().is_ok());
    let line = format!("\"{key}\": \"{}\"", wrapper.display());
    let written = fs::read_to_string(&code).unwrap();
    assert!(written.contains(&line), "{written}");
    assert!(written.contains("// my settings"), "{written}");
    assert!(fs::read_to_string(&cursor).unwrap().contains(&line));
    let listed = world.ok(&home, &["ide"]);
    assert!(listed.contains("through Orberus"), "{listed}");
    assert!(!listed.contains("skips Orberus"), "{listed}");

    // With the extension installed, doctor has nothing to say about them.
    world.bundled();
    let doctor = text(&world.orberus(&home, &["doctor"]));
    assert!(
        doctor.contains("the wrapper your editors start is installed"),
        "{doctor}"
    );
    assert!(!doctor.contains("skips Orberus"), "{doctor}");

    // `off` gives each file back as it was.
    world.ok(&home, &["ide", "off"]);
    assert_eq!(fs::read_to_string(&code).unwrap(), before);
    assert!(!fs::read_to_string(&cursor).unwrap().contains(key));
    let doctor = text(&world.orberus(&home, &["doctor"]));
    assert!(
        doctor.contains("skips Orberus in Cursor, VS Code"),
        "{doctor}"
    );
}

#[test]
fn ide_can_be_turned_on_for_one_editor_only() {
    let world = World::new();
    let home = world.home.clone();
    let code = world.editor("Code", "{}");
    let cursor = world.editor("Cursor", "{}");
    let key = "claudeCode.claudeProcessWrapper";

    // By the name the list shows, in any case.
    world.ok(&home, &["ide", "on", "vs code"]);
    assert!(fs::read_to_string(&code).unwrap().contains(key));
    assert_eq!(fs::read_to_string(&cursor).unwrap(), "{}");

    // A name that is not there changes nothing and says what is.
    let error = refused(&world, &home, &["ide", "on", "zed"]);
    assert!(error.contains("found: Cursor, VS Code"), "{error}");
    assert_eq!(fs::read_to_string(&cursor).unwrap(), "{}");

    world.ok(&home, &["ide", "off", "Cursor"]);
    assert!(fs::read_to_string(&code).unwrap().contains(key));
    world.ok(&home, &["ide", "off", "VS Code"]);
    assert!(!fs::read_to_string(&code).unwrap().contains(key));
}

#[test]
fn an_editor_without_the_extension_is_not_said_to_skip_orberus() {
    let world = World::new();
    let home = world.home.clone();
    let code = world.editor("Code", "{}");
    let cursor = world.editor("Cursor", "{}");
    world.app("Visual Studio Code.app", "Code", ".vscode");
    world.app("Cursor.app", "Cursor", ".cursor");
    // The extension is installed in VS Code only.
    world.bundled();
    let key = "claudeCode.claudeProcessWrapper";

    let listed = world.ok(&home, &["ide"]);
    let line = |name: &str| listed.lines().find(|line| line.contains(name)).unwrap();
    assert!(line("VS Code").contains("skips Orberus"), "{listed}");
    assert!(line("Cursor").contains("not installed"), "{listed}");
    let doctor = text(&world.orberus(&home, &["doctor"]));
    assert!(doctor.contains("skips Orberus in VS Code\n"), "{doctor}");

    // Without a name, only the editor that has the extension is written.
    let done = world.ok(&home, &["ide", "on"]);
    assert!(
        done.contains("Cursor: the Claude Code extension is not installed there"),
        "{done}"
    );
    assert!(fs::read_to_string(&code).unwrap().contains(key));
    assert_eq!(fs::read_to_string(&cursor).unwrap(), "{}");
    // Nothing is left to say, here or in doctor.
    let listed = world.ok(&home, &["ide"]);
    assert!(!listed.contains("orberus ide on"), "{listed}");
    let doctor = text(&world.orberus(&home, &["doctor"]));
    assert!(!doctor.contains("skips Orberus"), "{doctor}");

    // Asked for by its name, it is prepared for the day the extension comes.
    world.ok(&home, &["ide", "on", "cursor"]);
    assert!(fs::read_to_string(&cursor).unwrap().contains(key));
}

#[test]
fn ide_leaves_another_wrapper_alone() {
    let world = World::new();
    let home = world.home.clone();
    let before = "{\n    \"claudeCode.claudeProcessWrapper\": \"/opt/company/wrap\"\n}\n";
    let code = world.editor("Code", before);

    let output = world.orberus(&home, &["ide", "on"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("another program"),
        "{}",
        text(&output)
    );
    world.ok(&home, &["ide", "off"]);
    world.ok(&home, &["setup"]);
    world.ok(&home, &["uninstall", "--yes"]);
    assert_eq!(fs::read_to_string(&code).unwrap(), before);
}

#[test]
fn uninstall_takes_the_wrapper_out_of_the_editors() {
    let world = World::new();
    let home = world.home.clone();
    let before = "{\n    \"editor.fontSize\": 13\n}\n";
    let code = world.editor("Code", before);
    world.ok(&home, &["setup"]);
    let wrapper = world.root.join("ide/claude-wrapper");
    assert!(wrapper.symlink_metadata().is_ok(), "setup installs it");
    world.ok(&home, &["ide", "on"]);
    assert_ne!(fs::read_to_string(&code).unwrap(), before);

    let done = world.ok(&home, &["uninstall", "--yes"]);
    assert!(done.contains("VS Code"), "{done}");
    // The editor no longer names the wrapper, and the wrapper is gone.
    assert_eq!(fs::read_to_string(&code).unwrap(), before);
    assert!(wrapper.symlink_metadata().is_err());
    assert!(!world.root.join("ide").exists());
}

#[test]
fn a_pause_sets_every_rule_aside_until_resumed() {
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
    let repo = world.repo("code/app", "git@github.com:acme/app.git");
    let work_dir = world.root.join("profiles/work");
    let by_rule = format!("dir={}\nsession=work\nargs=--print\n", work_dir.display());
    assert_eq!(text(&world.shim(&repo, &["--print"], &[])), by_rule);

    let said = world.ok(&home, &["pause"]);
    assert!(said.contains("paused"), "{said}");
    // The tool starts untouched: what was in the environment stays, and
    // neither the locked rule nor an explicit choice is looked at.
    let untouched = "dir=unset\nsession=unset\nargs=--print\n";
    assert_eq!(text(&world.shim(&repo, &["--print"], &[])), untouched);
    let output = world.shim(&repo, &["--print"], &[("ORBERUS_PROFILE", "perso")]);
    assert!(
        output.status.success(),
        "a locked rule refuses nothing while paused"
    );
    assert_eq!(text(&output), untouched);
    // The editors' wrapper too.
    let copy = world.bundled();
    let output = world.wrapper(&repo, &[copy.to_str().unwrap(), "--print"], &[]);
    assert_eq!(
        text(&output),
        "bundled dir=unset session=unset args=--print\n"
    );
    // A config that cannot be read does not stop a paused Orberus.
    let config = world.root.join("config.toml");
    let good = fs::read_to_string(&config).unwrap();
    fs::write(&config, "not = [valid").unwrap();
    assert_eq!(text(&world.shim(&repo, &["--print"], &[])), untouched);
    fs::write(&config, good).unwrap();

    // `status` still says what the rules give, and that they are set aside.
    let status = world.ok(&repo, &["status"]);
    assert!(
        status.contains("work") && status.contains("paused"),
        "{status}"
    );

    world.ok(&home, &["resume"]);
    assert_eq!(text(&world.shim(&repo, &["--print"], &[])), by_rule);
}

#[test]
fn uninstall_ends_a_pause() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    world.ok(&home, &["pause"]);
    assert!(world.root.join("paused").exists());
    world.ok(&home, &["uninstall", "--yes"]);
    assert!(!world.root.join("paused").exists());
}

#[test]
fn uninstall_stops_opening_the_app_at_login() {
    let world = World::new();
    let home = world.home.clone();
    world.ok(&home, &["setup"]);
    // What the menu bar app writes when it is set to start with the Mac.
    let agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&agents).unwrap();
    let ours = agents.join("com.github.tzdev0.orberus.plist");
    let other = agents.join("com.example.other.plist");
    fs::write(&ours, "<plist/>").unwrap();
    fs::write(&other, "<plist/>").unwrap();

    let done = world.ok(&home, &["uninstall", "--yes"]);
    assert!(done.contains("no longer opens at login"), "{done}");
    assert!(!ours.exists());
    // Another program's file is not ours to remove.
    assert!(other.exists());
}
