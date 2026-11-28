//! Claude Desktop, on one profile at a time.
//!
//! Claude Desktop keeps everything about the account it is signed in to in
//! one data folder, `~/Library/Application Support/Claude`. Its help page
//! says how to hold two accounts of different addresses: log out, log in.
//! It is built on Electron, though, and takes Chromium's `--user-data-dir`
//! switch, which starts it on another data folder, with the sign-in that is
//! kept there. So each profile has a data folder, and switching account is
//! closing Claude Desktop and opening it again on the other profile's
//! folder ([`switch`]). This is not a documented feature of Claude Desktop.
//!
//! From outside the app, nothing can change the account of a Claude Desktop
//! that is open, short of modifying the app or handling its session
//! cookies. This tool does neither.
//!
//! Claude Desktop is started on a profile this way:
//!
//! ```text
//! open -n -a Claude.app --env CLAUDE_CONFIG_DIR=<the profile's Claude Code folder>
//!      --args --user-data-dir=~/.orberus/desktop/<profile>
//! ```
//!
//! The profile that reuses `~/.claude` gets Claude Desktop as it is: no
//! switch, no variable, its usual data folder. Nothing changes for the
//! Claude Desktop people already have.
//!
//! Tried on 2026-11-28 with Claude Desktop 7.37095.0, on an empty data
//! folder, never signed in. These findings shape this file:
//!
//! - **Nothing stops two copies on one data folder.** Started twice with
//!   the same switch, Claude Desktop ran twice. So before starting it on a
//!   profile we look for a copy that already runs on that folder, and bring
//!   it to the front instead.
//! - **Two copies on two folders run side by side**, each with its own icon
//!   in the Dock. A switch does not want that: the one that is open is
//!   asked to quit, and Claude Desktop is started on the other folder once
//!   it is gone, so one runs at any moment. One case needs the other order:
//!   a switch asked from inside the Claude Desktop that has to close (typed
//!   in its terminal, or run by a Claude Code session of its Code tab).
//!   Closing the app ends that terminal, and this program with it, before
//!   anything was started. There, the new one is started first and two run
//!   for a moment ([`inside`]).
//! - **macOS can ask an app to quit, by its number**, the way its Quit menu
//!   does, without the permission to "control" it. An idle copy was gone
//!   one second later. The app may also ask the user something first: it
//!   is never forced.
//! - **`open` hands its whole environment to the app.** Started from a
//!   terminal inside a Claude Code session, Claude Desktop would inherit
//!   that session's variables. It is started with a short, fixed
//!   environment.
//! - **The Code tab follows `CLAUDE_CONFIG_DIR`** (read in the app's code,
//!   which reads the variable where it would use `~/.claude`). On a
//!   profile, Claude Desktop is given the profile's own Claude Code folder,
//!   so its Code tab does not write in `~/.claude`, which belongs to
//!   another account. Not seen with a signed-in copy yet.
//!
//! Nothing here is on the shim's path: `claude` in a terminal never looks
//! at any of this.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::{self, Config};
use crate::paths::Env;
use crate::uninstall::app_folders;

/// What Claude Desktop calls itself in its Info.plist.
const IDENTIFIER: &str = "com.anthropic.claudefordesktop";

/// The switch that names a copy's data folder.
const SWITCH: &str = "--user-data-dir=";

/// The variable Claude Code, and Claude Desktop's Code tab, read their
/// folder from.
const CODE_FOLDER: &str = "CLAUDE_CONFIG_DIR";

/// What a copy is started with, out of our own environment: what an app
/// opened from the Finder has, and no more.
const KEPT: [&str; 8] = [
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "SSH_AUTH_SOCK",
    "COMMAND_MODE",
    "__CF_USER_TEXT_ENCODING",
];

/// The search path of an app opened from the Finder.
const FINDER_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// The Claude Desktop app installed on this Mac.
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// `/Applications/Claude.app`
    pub bundle: PathBuf,
    /// The program inside it, which is what a running copy is listed as.
    pub executable: PathBuf,
}

/// The value of a text key in an Info.plist written as XML.
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let (_, after) = xml.split_once(&format!("<key>{key}</key>"))?;
    let value = after.trim_start().strip_prefix("<string>")?;
    let (text, _) = value.split_once("</string>")?;
    Some(text.trim().replace("&amp;", "&"))
}

/// The Info.plist of an app as XML, when it mentions Claude Desktop at all.
fn info(bundle: &Path) -> Option<String> {
    let file = bundle.join("Contents/Info.plist");
    let bytes = fs::read(&file).ok()?;
    // The file is XML or binary. In both, the identifier is there in plain
    // bytes: this look spares reading every other app any further.
    let wanted = IDENTIFIER.as_bytes();
    if !bytes.windows(wanted.len()).any(|window| window == wanted) {
        return None;
    }
    match String::from_utf8(bytes) {
        Ok(text) if text.contains("<plist") => Some(text),
        // A binary file: `plutil`, which ships with macOS, rewrites it.
        _ => {
            let output = Command::new("plutil")
                .args(["-convert", "xml1", "-o", "-"])
                .arg(&file)
                .output()
                .ok()?;
            String::from_utf8(output.stdout).ok()
        }
    }
}

fn app_in(folders: &[PathBuf]) -> Option<App> {
    for folder in folders {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        // Sorted, so the same app is found each time when there are two.
        let mut bundles: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|kind| kind == "app"))
            .collect();
        bundles.sort();
        for bundle in bundles {
            let Some(xml) = info(&bundle) else {
                continue;
            };
            // The app is known by its identifier, never by its name: a file
            // that only mentions Claude Desktop is not it.
            if plist_string(&xml, "CFBundleIdentifier").as_deref() != Some(IDENTIFIER) {
                continue;
            }
            let Some(program) = plist_string(&xml, "CFBundleExecutable") else {
                continue;
            };
            return Some(App {
                executable: bundle.join("Contents/MacOS").join(program),
                bundle,
            });
        }
    }
    None
}

/// Claude Desktop, when it is installed in an Applications folder.
pub fn app(env: &Env) -> Option<App> {
    app_in(&app_folders(env))
}

/// The folders where an app is looked for, to say where it was not found.
pub fn searched(env: &Env) -> Vec<PathBuf> {
    app_folders(env)
}

/// Where Claude Desktop keeps its data when nothing tells it otherwise.
pub fn default_data_dir(env: &Env) -> PathBuf {
    env.home
        .join("Library")
        .join("Application Support")
        .join("Claude")
}

/// Where the copies of the other profiles keep theirs.
fn copies_dir(env: &Env) -> PathBuf {
    env.root.join("desktop")
}

/// The data folder of a profile's Claude Desktop. `None` is Claude
/// Desktop's own folder: the profile that reuses `~/.claude` also reuses
/// the Claude Desktop that was already there.
///
/// The folder goes by the profile's name, whatever Claude Code folder the
/// profile was given in the config.
pub fn data_dir(env: &Env, config: &Config, name: &str) -> Result<Option<PathBuf>> {
    // A name written by hand in the config could be `..`: it becomes a
    // folder name here, and that folder can be deleted.
    config::validate_name(name)?;
    Ok(config::profile_dir(env, config, name)?.map(|_| copies_dir(env).join(name)))
}

/// Every data folder a Claude Desktop may have written in: its own, then
/// one per profile that was opened.
pub fn data_dirs(env: &Env) -> Vec<PathBuf> {
    let mut dirs = vec![default_data_dir(env)];
    if let Ok(entries) = fs::read_dir(copies_dir(env)) {
        let mut copies: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.is_dir())
            .collect();
        copies.sort();
        dirs.extend(copies);
    }
    dirs
}

/// A copy of Claude Desktop that is running.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    pub pid: u32,
    /// What it was started with, after the program's path.
    arguments: String,
}

impl Running {
    /// True when this copy runs on that data folder. `None` asks for the
    /// copy on Claude Desktop's own folder: the one started with no switch.
    pub fn uses(&self, data: Option<&Path>) -> bool {
        let Some(data) = data else {
            return !self.arguments.contains(SWITCH);
        };
        names_folder(&self.arguments, data)
    }
}

/// True when a command line carries the switch for exactly that folder:
/// `…/desktop/work` is not `…/desktop/work2`.
fn names_folder(arguments: &str, data: &Path) -> bool {
    let wanted = format!("{SWITCH}{}", data.display());
    arguments.match_indices(&wanted).any(|(at, _)| {
        let after = &arguments[at + wanted.len()..];
        after.is_empty() || after.starts_with(' ')
    })
}

/// The copies of the app among the lines `ps` printed, one program per
/// line: its number, then its command line.
fn running_in(listing: &str, executable: &Path) -> Vec<Running> {
    let executable = executable.to_string_lossy();
    listing
        .lines()
        .filter_map(|line| {
            let (pid, command) = line.trim_start().split_once(' ')?;
            let pid = pid.parse().ok()?;
            let arguments = command.trim_start().strip_prefix(executable.as_ref())?;
            // The app's helper programs live in another folder of the app
            // and never start with this path. The check below keeps out a
            // program whose name merely begins like ours.
            if !(arguments.is_empty() || arguments.starts_with(' ')) {
                return None;
            }
            Some(Running {
                pid,
                arguments: arguments.to_string(),
            })
        })
        .collect()
}

/// Every running program with its command line, as `ps` prints them.
///
/// `ps` and not `pgrep`: `pgrep` leaves out its own ancestors, so from a
/// terminal inside Claude Desktop it would not see Claude Desktop.
fn listing() -> Result<String> {
    let output = Command::new("ps")
        // `ww`: never cut a line, the data folder is at its end.
        .args(["-axww", "-o", "pid=,args="])
        .output()
        .context("cannot run `ps`")?;
    if !output.status.success() {
        bail!("`ps` could not list the running programs");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The copies of Claude Desktop that are running now.
pub fn running(app: &App) -> Result<Vec<Running>> {
    Ok(running_in(&listing()?, &app.executable))
}

/// True when the program numbered `from` was started by one of `pids`,
/// directly or through other programs. `table` is what `ps` printed: one
/// program per line, its number, then the number of the one that started it.
fn started_by(table: &str, from: u32, pids: &[u32]) -> bool {
    let parent: HashMap<u32, u32> = table
        .lines()
        .filter_map(|line| {
            let mut numbers = line.split_whitespace();
            // `?` inside a closure that returns an `Option`: a line that is
            // not two numbers is skipped.
            Some((numbers.next()?.parse().ok()?, numbers.next()?.parse().ok()?))
        })
        .collect();
    let mut at = from;
    // Programs start and stop while `ps` reads them, so nothing promises
    // that the table has no loop: the walk up is bounded.
    for _ in 0..64 {
        let Some(&up) = parent.get(&at) else {
            return false;
        };
        if pids.contains(&up) {
            return true;
        }
        at = up;
    }
    false
}

/// True when this very program runs inside one of those Claude Desktops:
/// typed in its terminal, or run by a Claude Code session of its Code tab.
/// Such a Claude Desktop cannot be closed first: this program would end
/// with it.
///
/// When `ps` cannot tell, the answer is yes. The order chosen for "inside"
/// works everywhere; it only shows two Claude Desktops for a moment.
fn inside(pids: &[u32]) -> bool {
    if pids.is_empty() {
        return false;
    }
    let table = Command::new("ps").args(["-axo", "pid=,ppid="]).output();
    match table {
        Ok(output) if output.status.success() => started_by(
            &String::from_utf8_lossy(&output.stdout),
            std::process::id(),
            pids,
        ),
        _ => true,
    }
}

/// Where a profile's Claude Desktop stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// A copy is running on the profile's data folder.
    Open(u32),
    /// It was opened before: its data folder is there.
    Closed,
    /// Never opened. Its first opening will ask to sign in.
    New,
}

/// The state of a profile's copy, given the copies that are running.
pub fn state(env: &Env, config: &Config, name: &str, copies: &[Running]) -> Result<State> {
    let data = data_dir(env, config, name)?;
    if let Some(copy) = copies.iter().find(|copy| copy.uses(data.as_deref())) {
        return Ok(State::Open(copy.pid));
    }
    let folder = data.unwrap_or_else(|| default_data_dir(env));
    Ok(if folder.is_dir() {
        State::Closed
    } else {
        State::New
    })
}

/// The command that starts a copy: `open`, with the environment and the
/// arguments described at the top of this file.
fn launcher(app: &App, data: Option<&Path>, code: Option<&Path>) -> Command {
    let mut command = Command::new("/usr/bin/open");
    // `open` gives the app everything in its own environment. Ours may be
    // a Claude Code session's: start from nothing and add back the basics.
    command.env_clear();
    for name in KEPT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command.env("PATH", FINDER_PATH);
    // `-n`: a new copy even though the app is running. We only get here
    // after checking that none runs on this data folder.
    command.arg("-n").arg("-a").arg(&app.bundle);
    if let Some(code) = code {
        // An `OsString` and not a `String`: a path need not be valid text.
        let mut variable = std::ffi::OsString::from(format!("{CODE_FOLDER}="));
        variable.push(code);
        command.arg("--env").arg(variable);
    }
    if let Some(data) = data {
        let mut switch = std::ffi::OsString::from(SWITCH);
        switch.push(data);
        command.arg("--args").arg(switch);
    }
    command.stdout(Stdio::null()).stderr(Stdio::null());
    command
}

/// Asks macOS to bring one running app to the front, by its number, once
/// it has finished starting. Returns false when macOS did not do it.
///
/// `osascript` ships with macOS. The script calls AppKit directly instead
/// of sending the app a message, so macOS does not ask the user to let us
/// "control" another app. Tried on macOS 27: 0.06 s.
fn show(pid: u32) -> Result<bool> {
    const SCRIPT: &str = r#"ObjC.import("AppKit");
function run(argv) {
  var pid = parseInt(argv[0], 10);
  for (var i = 0; i < 50; i++) {
    var app = $.NSRunningApplication.runningApplicationWithProcessIdentifier(pid);
    if (app.isNil()) return "gone";
    if (app.finishedLaunching) return app.activateWithOptions(3) ? "shown" : "refused";
    delay(0.1);
  }
  return "refused";
}"#;
    let output = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .arg(pid.to_string())
        .output()
        .context("cannot run `osascript`")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "shown")
}

/// Asks macOS to quit one running app, by its number, as its Quit menu
/// does: the app closes its windows and saves what it must, and may ask the
/// user something first. Nothing is forced. Returns false when macOS knows
/// no app by that number, or would not ask.
///
/// Like [`show`], this calls AppKit and sends the app no message of ours,
/// so macOS asks for no permission.
///
/// Nothing in it is about Claude Desktop: `uninstall` asks our own menu bar
/// app to quit the same way.
pub(crate) fn quit(pid: u32) -> Result<bool> {
    const SCRIPT: &str = r#"ObjC.import("AppKit");
function run(argv) {
  var app = $.NSRunningApplication.runningApplicationWithProcessIdentifier(parseInt(argv[0], 10));
  if (app.isNil()) return "gone";
  return app.terminate ? "asked" : "refused";
}"#;
    let output = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .arg(pid.to_string())
        .output()
        .context("cannot run `osascript`")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "asked")
}

/// How long a copy that was asked to quit is waited for: 10 seconds, in
/// steps of 200 ms. An idle one took one second.
const QUIT_STEPS: u32 = 50;

/// Waits for those copies to be gone, and returns the ones that are still
/// running when the wait is over. A list that cannot be read counts as
/// "still running": saying "closed" without knowing would be a guess.
fn still_running(app: &App, pids: &[u32]) -> Vec<u32> {
    let mut left = pids.to_vec();
    for _ in 0..QUIT_STEPS {
        if left.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
        if let Ok(copies) = running(app) {
            left.retain(|pid| copies.iter().any(|copy| copy.pid == *pid));
        }
    }
    left
}

/// Asks those Claude Desktops to quit and waits for them. True when they
/// are all gone, and at once when there is none.
fn close(app: &App, pids: &[u32]) -> Result<bool> {
    for pid in pids {
        // "No such app" is fine: that one went away by itself.
        quit(*pid)?;
    }
    Ok(still_running(app, pids).is_empty())
}

/// Waits for the copy on that data folder to be listed, a few seconds at
/// most, and returns its number.
fn started(app: &App, data: Option<&Path>) -> Option<u32> {
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(200));
        let copies = running(app).ok()?;
        if let Some(copy) = copies.iter().find(|copy| copy.uses(data)) {
            return Some(copy.pid);
        }
    }
    None
}

/// Creates a data folder that only its owner can open, as Claude Desktop
/// does for its own.
fn create_private(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// The profile a running copy is on: the one whose data folder it runs on.
/// `None` for a copy on a folder that is no profile's, such as the Claude
/// Desktop opened from the Dock when no profile reuses `~/.claude`.
pub fn owner(env: &Env, config: &Config, copy: &Running) -> Option<String> {
    config
        .profiles
        .keys()
        .find(|name| data_dir(env, config, name).is_ok_and(|data| copy.uses(data.as_deref())))
        .cloned()
}

/// What a switch to that profile closes, among the copies that are
/// running: every Claude Desktop on another data folder. For each, the
/// profile it is on, or `None`. Empty when nothing has to close.
///
/// This is what to tell the user before a switch: what runs in a Claude
/// Desktop that closes stops with it.
pub fn in_the_way(
    env: &Env,
    config: &Config,
    name: &str,
    copies: &[Running],
) -> Result<Vec<Option<String>>> {
    let data = data_dir(env, config, name)?;
    Ok(copies
        .iter()
        .filter(|copy| !copy.uses(data.as_deref()))
        .map(|copy| owner(env, config, copy))
        .collect())
}

/// What [`switch`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Switched {
    /// True when Claude Desktop was started on the profile; false when it
    /// was already running there and was only brought to the front.
    pub started: bool,
    /// How many other Claude Desktops were closed.
    pub closed: usize,
}

/// One switch at a time in this program. Between "nothing runs on this
/// folder" and the new copy being listed, a second call would start a
/// second copy on the same folder. The menu bar app can be asked twice in a
/// row: the second call waits here, then finds what the first one did.
///
/// A `static` with a `Mutex` inside: one lock for the whole program, which
/// Rust lets us build at compile time (`Mutex::new` is a `const fn`).
static SWITCHING: Mutex<()> = Mutex::new(());

/// Puts Claude Desktop on a profile, and leaves one Claude Desktop open.
///
/// When it already runs on the profile, it is brought to the front and the
/// others are asked to quit. Otherwise the others are asked to quit first,
/// and Claude Desktop is started on the profile's data folder once they
/// are gone: one runs at any moment, with one icon in the Dock. Asked from
/// [`inside`] a Claude Desktop that has to close, the order is the other
/// one, new first: the top of this file says why. This program may then be
/// ended while it waits for the others to be gone. By then the new one
/// runs and the others were asked: nothing is left half-done.
pub fn switch(env: &Env, config: &Config, name: &str) -> Result<Switched> {
    let data = data_dir(env, config, name)?;
    let Some(app) = app(env) else {
        bail!("{}", not_found(env));
    };
    // The lock is "poisoned" when a call panicked while holding it. It
    // guards no data, so there is nothing left half-written: carry on.
    // `_one_at_a_time` keeps it until this function returns.
    let _one_at_a_time = SWITCHING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // An error here stops everything: starting a copy without knowing
    // whether one already runs could put two on the same data folder.
    let copies = running(&app)?;
    let mine = copies
        .iter()
        .find(|copy| copy.uses(data.as_deref()))
        .map(|copy| copy.pid);
    let others: Vec<u32> = copies
        .iter()
        .filter(|copy| !copy.uses(data.as_deref()))
        .map(|copy| copy.pid)
        .collect();

    let in_front = match mine {
        // Already on this profile: nothing to start, so no order to choose.
        Some(pid) => {
            let shown = show(pid)?;
            if !close(&app, &others)? {
                bail!(
                    "Claude Desktop is on profile '{name}', but the other one did not \
                     quit: it may be asking you something. Answer it, or quit it yourself"
                );
            }
            shown
        }
        None => {
            // Whatever can fail without touching Claude Desktop fails here,
            // before anything is closed.
            if let Some(dir) = &data {
                create_private(dir).with_context(|| format!("cannot create {}", dir.display()))?;
            }
            let code = config::profile_dir(env, config, name)?;
            let mut open = launcher(&app, data.as_deref(), code.as_deref());

            let close_first = !others.is_empty() && !inside(&others);
            if close_first && !close(&app, &others)? {
                bail!(
                    "Claude Desktop did not quit: it may be asking you something. \
                     Answer it, then switch again. Nothing was opened on profile '{name}'"
                );
            }
            let opened = open.status().context("cannot run `open`")?.success();
            // Not `started(…)` when `open` failed: there is nothing to wait for.
            let pid = if opened {
                started(&app, data.as_deref())
            } else {
                None
            };
            let Some(pid) = pid else {
                if close_first {
                    bail!(
                        "Claude Desktop was closed, but it did not start on profile \
                         '{name}': switch again, or open Claude Desktop yourself"
                    );
                }
                // New first: an app that failed to start must not cost the
                // one that was open.
                bail!(
                    "Claude Desktop did not start on profile '{name}': \
                     nothing else was closed"
                );
            };
            // Started from a program that is not in front, it may stay
            // behind the other windows. Bringing it forward is a courtesy:
            // it is running either way.
            let _ = show(pid);
            if !close_first && !close(&app, &others)? {
                bail!(
                    "Claude Desktop is on profile '{name}', but the one that was open \
                     did not quit: it may be asking you something. Answer it, or quit \
                     it yourself"
                );
            }
            true
        }
    };
    // Saying "brought to the front" when macOS did not do it would send
    // the user looking for a window that did not move.
    if !in_front {
        bail!(
            "Claude Desktop is on profile '{name}', but macOS did not bring it \
             to the front: pick its window yourself"
        );
    }
    Ok(Switched {
        started: mine.is_none(),
        closed: others.len(),
    })
}

/// The sentence for a Mac without Claude Desktop.
pub fn not_found(env: &Env) -> String {
    let folders: Vec<String> = searched(env)
        .iter()
        .map(|folder| crate::view::short_path(env, folder))
        .collect();
    if folders.is_empty() {
        return "Claude Desktop profiles are for macOS only".to_string();
    }
    format!("Claude Desktop was not found in {}", folders.join(" or "))
}

/// Deletes the data folder of a profile's Claude Desktop: its sign-in
/// there and everything that copy kept. Returns false when there was none.
///
/// Claude Desktop's own folder is never deleted, and neither is a folder
/// a copy is running on: it would keep writing in it.
pub fn forget(env: &Env, config: &Config, name: &str) -> Result<bool> {
    let Some(dir) = data_dir(env, config, name)? else {
        return Ok(false);
    };
    if !dir.is_dir() {
        return Ok(false);
    }
    // Any program at all that was started on this folder counts, the app's
    // helpers included: it need not be the app we would find today.
    // Only a Mac has such copies; elsewhere there is nothing to look for.
    let in_use =
        cfg!(target_os = "macos") && listing()?.lines().any(|line| names_folder(line, &dir));
    if in_use {
        bail!("Claude Desktop is open on profile '{name}': quit it, then try again");
    }
    fs::remove_dir_all(&dir).with_context(|| format!("cannot delete {}", dir.display()))?;
    Ok(true)
}

/// The profiles whose own copy of Claude Desktop is running now, by the
/// name of their data folder. Asked before every one of those folders is
/// deleted at once.
pub fn open_copies(env: &Env) -> Result<Vec<String>> {
    let copies: Vec<PathBuf> = data_dirs(env).into_iter().skip(1).collect();
    if copies.is_empty() || !cfg!(target_os = "macos") {
        return Ok(Vec::new());
    }
    let listing = listing()?;
    Ok(copies
        .iter()
        .filter(|dir| listing.lines().any(|line| names_folder(line, dir)))
        .filter_map(|dir| Some(dir.file_name()?.to_string_lossy().into_owned()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    fn world() -> (tempfile::TempDir, Env, Config) {
        let temp = tempfile::tempdir().unwrap();
        let env = Env {
            home: temp.path().join("home"),
            root: temp.path().join("home/.orberus"),
        };
        let mut config = Config::default();
        config.profiles.insert(
            "perso".to_string(),
            Profile {
                native: true,
                dir: None,
            },
        );
        config
            .profiles
            .insert("work".to_string(), Profile::default());
        (temp, env, config)
    }

    /// An app in `folder`, with what its Info.plist says.
    fn install(folder: &Path, name: &str, identifier: &str, program: &str) -> PathBuf {
        let bundle = folder.join(name);
        fs::create_dir_all(bundle.join("Contents")).unwrap();
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\"?>\n<plist version=\"1.0\"><dict>\n\
                 <key>CFBundleExecutable</key>\n\t<string>{program}</string>\n\
                 <key>CFBundleIdentifier</key>\n\t<string>{identifier}</string>\n\
                 </dict></plist>\n"
            ),
        )
        .unwrap();
        bundle
    }

    const EXECUTABLE: &str = "/Applications/Claude.app/Contents/MacOS/Claude";

    fn copies(listing: &str) -> Vec<Running> {
        running_in(listing, Path::new(EXECUTABLE))
    }

    #[test]
    fn the_app_is_known_by_its_identifier_not_its_name() {
        let temp = tempfile::tempdir().unwrap();
        let apps = temp.path().to_path_buf();
        // Same name, another app; and an app that only mentions ours.
        install(&apps, "Claude.app", "com.example.claude", "Claude");
        install(
            &apps,
            "Helper.app",
            "com.example.helper",
            "com.anthropic.claudefordesktop",
        );
        assert_eq!(app_in(std::slice::from_ref(&apps)), None);

        let bundle = install(&apps, "Renamed.app", IDENTIFIER, "Claude");
        assert_eq!(
            app_in(&[temp.path().join("nowhere"), apps]),
            Some(App {
                executable: bundle.join("Contents/MacOS/Claude"),
                bundle,
            })
        );
    }

    #[test]
    fn the_native_profile_keeps_claude_desktops_own_folder() {
        let (_temp, env, config) = world();
        assert_eq!(data_dir(&env, &config, "perso").unwrap(), None);
        assert_eq!(
            data_dir(&env, &config, "work").unwrap(),
            Some(env.root.join("desktop/work"))
        );
        assert!(data_dir(&env, &config, "nobody").is_err());
    }

    #[test]
    fn a_name_that_is_not_a_profile_name_is_never_a_folder() {
        let (_temp, env, mut config) = world();
        // Written by hand in the config: it would name `~/.orberus` itself.
        config.profiles.insert("..".to_string(), Profile::default());
        assert!(data_dir(&env, &config, "..").is_err());
        assert!(forget(&env, &config, "..").is_err());
    }

    #[test]
    fn running_copies_are_told_apart_by_their_data_folder() {
        let listing = "\
  501 /Applications/Claude.app/Contents/MacOS/Claude
  502 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/me/.orberus/desktop/work
  503 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/me/.orberus/desktop/work2 --startup
  504 /Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper --type=gpu --user-data-dir=/Users/me/.orberus/desktop/work
  505 /Applications/Claude.app/Contents/MacOS/ClaudeOther
  506 /usr/bin/vim /Applications/Claude.app/Contents/MacOS/Claude
";
        let found = copies(listing);
        let pids: Vec<u32> = found.iter().map(|copy| copy.pid).collect();
        assert_eq!(pids, [501, 502, 503]);

        let on = |dir: Option<&str>| -> Vec<u32> {
            found
                .iter()
                .filter(|copy| copy.uses(dir.map(Path::new)))
                .map(|copy| copy.pid)
                .collect()
        };
        assert_eq!(on(None), [501]);
        assert_eq!(on(Some("/Users/me/.orberus/desktop/work")), [502]);
        assert_eq!(on(Some("/Users/me/.orberus/desktop/work2")), [503]);
        assert!(on(Some("/Users/me/.orberus/desktop")).is_empty());
    }

    #[test]
    fn a_home_folder_with_a_space_is_still_matched() {
        let listing = "  7 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/my name/.orberus/desktop/work\n";
        let found = copies(listing);
        assert!(found[0].uses(Some(Path::new("/Users/my name/.orberus/desktop/work"))));
        assert!(!found[0].uses(None));
    }

    #[test]
    fn state_says_open_closed_or_never_opened() {
        let (_temp, env, config) = world();
        let state_of =
            |name: &str, listing: &str| state(&env, &config, name, &copies(listing)).unwrap();
        assert_eq!(state_of("work", ""), State::New);
        assert_eq!(state_of("perso", ""), State::New);

        fs::create_dir_all(env.root.join("desktop/work")).unwrap();
        fs::create_dir_all(default_data_dir(&env)).unwrap();
        assert_eq!(state_of("work", ""), State::Closed);
        assert_eq!(state_of("perso", ""), State::Closed);

        let work = format!(
            "  42 {EXECUTABLE} --user-data-dir={}\n",
            env.root.join("desktop/work").display()
        );
        assert_eq!(state_of("work", &work), State::Open(42));
        // The copy of another profile is not Claude Desktop's own.
        assert_eq!(state_of("perso", &work), State::Closed);
        assert_eq!(
            state_of("perso", &format!("  41 {EXECUTABLE}\n")),
            State::Open(41)
        );
    }

    #[test]
    fn a_switch_closes_every_other_claude_desktop_and_says_whose() {
        let (_temp, env, mut config) = world();
        config
            .profiles
            .insert("client".to_string(), Profile::default());
        let on = |name: &str| env.root.join("desktop").join(name).display().to_string();
        let listing = format!(
            "  41 {EXECUTABLE}\n  42 {EXECUTABLE} --user-data-dir={}\n  43 {EXECUTABLE} --user-data-dir={}\n",
            on("work"),
            on("gone"),
        );
        let found = copies(&listing);
        let owners: Vec<Option<String>> = found
            .iter()
            .map(|copy| owner(&env, &config, copy))
            .collect();
        // Claude Desktop's own folder is the native profile's; a folder
        // left by a profile that was removed is nobody's.
        assert_eq!(
            owners,
            [Some("perso".to_string()), Some("work".to_string()), None]
        );

        let closes = |name: &str| in_the_way(&env, &config, name, &found).unwrap();
        assert_eq!(closes("work"), [Some("perso".to_string()), None]);
        assert_eq!(closes("perso"), [Some("work".to_string()), None]);
        assert_eq!(
            closes("client"),
            [Some("perso".to_string()), Some("work".to_string()), None]
        );
        // Already alone on the profile, or nothing running: nothing closes.
        let alone = copies(&format!("  41 {EXECUTABLE}\n"));
        assert!(in_the_way(&env, &config, "perso", &alone)
            .unwrap()
            .is_empty());
        assert!(in_the_way(&env, &config, "work", &[]).unwrap().is_empty());
        assert!(in_the_way(&env, &config, "nobody", &found).is_err());

        // Without a profile that reuses `~/.claude`, the Claude Desktop
        // opened from the Dock is no profile's: it closes all the same.
        config.profiles.remove("perso");
        assert_eq!(owner(&env, &config, &alone[0]), None);
        assert_eq!(in_the_way(&env, &config, "work", &alone).unwrap(), [None]);
    }

    #[test]
    fn a_program_knows_when_it_runs_inside_a_claude_desktop() {
        // Number, then the number of the program that started it. 400 is a
        // Claude Desktop with a shell (410) and a command in it (420); 900
        // is a terminal of its own with a command (910).
        let table =
            "    1     0\n  400     1\n  410   400\n  420   410\n  900     1\n  910   900\n";
        assert!(started_by(table, 410, &[400]));
        assert!(started_by(table, 420, &[400]));
        assert!(started_by(table, 420, &[777, 400]));
        // Started from another terminal, or from the menu bar app.
        assert!(!started_by(table, 910, &[400]));
        assert!(!started_by(table, 420, &[900]));
        // A program `ps` did not list, and nothing to be inside of.
        assert!(!started_by(table, 4242, &[400]));
        assert!(!started_by(table, 420, &[]));
        // A table that loops is not walked forever.
        assert!(!started_by("5 6\n6 5\n", 5, &[400]));
        // The real table: every program descends from the first one, and
        // with nothing to be inside of the answer needs no table.
        assert!(inside(&[1]));
        assert!(!inside(&[]));
    }

    #[test]
    fn a_copy_is_started_on_its_folders_with_a_clean_environment() {
        let app = App {
            bundle: PathBuf::from("/Applications/Claude.app"),
            executable: PathBuf::from(EXECUTABLE),
        };
        let command = launcher(
            &app,
            Some(Path::new("/Users/me/.orberus/desktop/work")),
            Some(Path::new("/Users/me/.orberus/profiles/work")),
        );
        let arguments: Vec<String> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            arguments,
            [
                "-n",
                "-a",
                "/Applications/Claude.app",
                "--env",
                "CLAUDE_CONFIG_DIR=/Users/me/.orberus/profiles/work",
                "--args",
                "--user-data-dir=/Users/me/.orberus/desktop/work",
            ]
        );
        // Nothing of ours reaches the app but the short list.
        let passed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_some())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        assert!(passed.contains(&"PATH".to_string()));
        assert!(passed
            .iter()
            .all(|name| name == "PATH" || KEPT.contains(&name.as_str())));

        // The profile that reuses `~/.claude`: Claude Desktop as it is.
        let plain = launcher(&app, None, None);
        let arguments: Vec<&std::ffi::OsStr> = plain.get_args().collect();
        assert_eq!(arguments, ["-n", "-a", "/Applications/Claude.app"]);
    }

    #[test]
    fn every_data_folder_is_listed_claude_desktops_own_first() {
        let (_temp, env, _config) = world();
        assert_eq!(data_dirs(&env), [default_data_dir(&env)]);
        fs::create_dir_all(env.root.join("desktop/work")).unwrap();
        fs::create_dir_all(env.root.join("desktop/client")).unwrap();
        fs::write(env.root.join("desktop/notes.txt"), "").unwrap();
        assert_eq!(
            data_dirs(&env),
            [
                default_data_dir(&env),
                env.root.join("desktop/client"),
                env.root.join("desktop/work"),
            ]
        );
    }

    #[test]
    fn forgetting_deletes_a_profiles_folder_and_never_the_default_one() {
        let (_temp, env, config) = world();
        assert!(!forget(&env, &config, "work").unwrap());

        let work = env.root.join("desktop/work");
        fs::create_dir_all(&work).unwrap();
        fs::write(work.join("Cookies"), "").unwrap();
        fs::create_dir_all(default_data_dir(&env)).unwrap();

        assert!(forget(&env, &config, "work").unwrap());
        assert!(!work.exists());
        assert!(!forget(&env, &config, "perso").unwrap());
        assert!(default_data_dir(&env).is_dir());
    }
}
