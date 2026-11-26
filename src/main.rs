//! `cprofile`: the command line tool. When this same binary is started under
//! the name `claude` (through the link created by `cprofile setup`), it acts
//! as the shim: it picks the account for the current folder, then hands over
//! to the real Claude Code.

mod ui;

use std::ffi::OsString;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};

use claude_profiles::config::{self, Config, Loaded, Rule};
use claude_profiles::disk;
use claude_profiles::git;
use claude_profiles::launch::{self, Target};
use claude_profiles::paths::Env;
use claude_profiles::purge;
use claude_profiles::resolve::{self, REPO_FILE};
use claude_profiles::retention::{self, Retention};
use claude_profiles::view::{account, location, short_path, when};

use ui::{Cell, Style, Ui};

const EXAMPLES: &str = "\
Examples:
  cprofile add                 create a profile, step by step
  cprofile use                 pick the profile for this folder
  cprofile rule add            send a repository or a folder to a profile
  cprofile status              which profile applies here, and why

Most commands ask for what is missing when you leave an argument out.";

#[derive(Parser)]
#[command(
    name = "cprofile",
    version,
    about = "The right Claude account for each folder",
    after_help = EXAMPLES
)]
struct Cli {
    // `Option`: with no subcommand we show an overview instead of an error.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Install the `claude` shim and put it in your PATH
    Setup,
    /// Create a profile (one profile = one Claude account)
    Add {
        /// Name of the profile. Asked when left out
        name: Option<String>,
        /// Reuse your current Claude Code login (the default ~/.claude folder)
        #[arg(long)]
        native: bool,
        /// Use a custom config folder for this profile
        #[arg(long, conflicts_with = "native")]
        dir: Option<String>,
        /// Keep its conversations this many days (Claude Code deletes them
        /// after 30 by default)
        #[arg(long, value_name = "DAYS")]
        keep_days: Option<u64>,
    },
    /// List profiles
    #[command(visible_alias = "ls")]
    List,
    /// Remove a profile and the rules that point to it
    #[command(visible_alias = "rm")]
    Remove {
        /// Name of the profile. Asked when left out
        name: Option<String>,
        /// Also delete its Claude Code folder (history, settings) and its
        /// login in the keychain. Cannot be undone
        #[arg(long)]
        purge: bool,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Show how much disk space each profile takes
    Disk,
    /// Show or set how long each profile keeps its conversations
    Retention {
        /// Name of the profile to change. Without it, all are listed
        name: Option<String>,
        /// Number of days to keep conversations. Asked when left out
        days: Option<u64>,
    },
    /// Set the profile used when no rule applies
    #[command(visible_alias = "global")]
    Default {
        /// Name of the profile. Asked when left out
        name: Option<String>,
    },
    /// Pin the current folder to a profile (writes a .claude-profile file)
    Use {
        /// Name of the profile. Asked when left out
        name: Option<String>,
    },
    /// Show which profile applies in the current folder, and why
    Status {
        /// Check what would happen with this explicit choice
        #[arg(long, short)]
        profile: Option<String>,
    },
    /// Start Claude Code, optionally forcing a profile
    Run {
        #[arg(long, short)]
        profile: Option<String>,
        /// Arguments passed to claude, after `--`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Manage the rules that pick a profile automatically
    Rule {
        #[command(subcommand)]
        command: RuleCommand,
    },
    /// Check that everything is wired correctly
    Doctor,
}

#[derive(Subcommand)]
enum RuleCommand {
    /// Add a rule. With no option, it is built from the current folder
    Add {
        /// Folder the rule covers, with everything below it
        #[arg(long)]
        path: Option<String>,
        /// Git remote pattern, for example "github.com/acme/*"
        #[arg(long)]
        git_remote: Option<String>,
        /// Profile to use where the rule matches. Asked when left out
        #[arg(long, short)]
        profile: Option<String>,
        /// Refuse any other profile where this rule matches
        #[arg(long)]
        locked: bool,
    },
    /// List rules in the order they are checked
    #[command(visible_alias = "ls")]
    List,
    /// Remove one of your rules, by its number in `rule list`
    #[command(visible_alias = "rm")]
    Remove {
        /// Number of the rule. Asked when left out
        number: Option<usize>,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
}

fn main() -> ExitCode {
    let mut args = std::env::args_os();
    let invoked = args.next().unwrap_or_default();
    let as_shim = Path::new(&invoked)
        .file_stem()
        .is_some_and(|stem| stem == "claude");

    let result = if as_shim { shim(args.collect()) } else { cli() };
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(error) => {
            // The shim names itself, since the user typed `claude` and would
            // not know where the message comes from otherwise.
            let prefix = if as_shim { "cprofile:" } else { "error:" };
            eprintln!("{} {error:#}", ui::paint_stderr(prefix, Style::Red));
            ExitCode::from(2)
        }
    }
}

/// Shim mode: every argument belongs to claude, nothing is parsed here.
fn shim(args: Vec<OsString>) -> Result<i32> {
    let env = Env::from_process()?;
    // Inside a session we already started: keep that session's account.
    if std::env::var_os(launch::SESSION_ENV).is_some() {
        let config = config::load(&env)
            .map(|loaded| loaded.config)
            .unwrap_or_default();
        let real = launch::find_real_claude(&env, &config)?;
        return launch::launch(&real, Target::Passthrough, args);
    }
    let loaded = config::load(&env)?;
    let explicit = std::env::var(launch::EXPLICIT_ENV)
        .ok()
        .filter(|name| !name.is_empty());
    run_claude(&env, &loaded, explicit, args)
}

fn run_claude(
    env: &Env,
    loaded: &Loaded,
    explicit: Option<String>,
    args: Vec<OsString>,
) -> Result<i32> {
    let real = launch::find_real_claude(env, &loaded.config)?;
    if loaded.is_empty() && explicit.is_none() {
        return launch::launch(&real, Target::Passthrough, args);
    }
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let Some(found) = resolve::resolve(env, loaded, &cwd, explicit.as_deref())? else {
        return launch::launch(&real, Target::Passthrough, args);
    };
    let dir = config::profile_dir(env, &loaded.config, &found.profile)?;
    if !loaded.config.quiet && std::io::stderr().is_terminal() {
        let notice = format!("cprofile: {} ({})", found.profile, found.reason.describe());
        eprintln!("{}", ui::paint_stderr(&notice, Style::Dim));
    }
    let target = Target::Profile {
        name: &found.profile,
        dir: dir.as_deref(),
    };
    launch::launch(&real, target, args)
}

fn cli() -> Result<i32> {
    let cli = Cli::parse();
    let env = Env::from_process()?;
    // One `Ui` for the whole command, lent (`&ui`) to whoever prints.
    let ui = Ui::new();
    let Some(command) = cli.command else {
        return overview(&ui, &env);
    };
    match command {
        Command::Setup => setup(&ui, &env),
        Command::Add {
            name,
            native,
            dir,
            keep_days,
        } => add(&ui, &env, name, native, dir, keep_days),
        Command::List => list(&ui, &env),
        Command::Remove { name, purge, yes } => remove(&ui, &env, name, purge, yes),
        Command::Disk => disk_usage(&ui, &env),
        Command::Retention { name, days } => retention_command(&ui, &env, name, days),
        Command::Default { name } => set_default(&ui, &env, name),
        Command::Use { name } => pin(&ui, &env, name),
        Command::Status { profile } => status(&ui, &env, profile),
        Command::Run { profile, args } => {
            let loaded = config::load(&env)?;
            run_claude(&env, &loaded, profile, args)
        }
        Command::Rule { command } => match command {
            RuleCommand::Add {
                path,
                git_remote,
                profile,
                locked,
            } => rule_add(&ui, &env, path, git_remote, profile, locked),
            RuleCommand::List => rule_list(&ui, &env),
            RuleCommand::Remove { number, yes } => rule_remove(&ui, &env, number, yes),
        },
        Command::Doctor => doctor(&ui, &env),
    }
}

// ---------------------------------------------------------------------------
// Small helpers shared by the commands
// ---------------------------------------------------------------------------

/// The account as a table cell or a status value.
fn account_cell(env: &Env, config: &Config, name: &str) -> Cell {
    match account(env, config, name) {
        Some(email) => Cell::new(email),
        None => Cell::styled("not logged in", Style::Yellow),
    }
}

fn unknown_profile(config: &Config, name: &str) -> anyhow::Error {
    if config.profiles.is_empty() {
        return anyhow!(
            "unknown profile '{name}': no profile exists yet, create one with: cprofile add {name}"
        );
    }
    let names: Vec<&str> = config.profiles.keys().map(String::as_str).collect();
    anyhow!("unknown profile '{name}' (available: {})", names.join(", "))
}

/// Returns the profile to act on: the one given on the command line when it
/// exists, otherwise the one picked in a list. In a script, where nobody can
/// answer, a missing name is an error that says what to type.
fn pick_profile(ui: &Ui, config: &Config, given: Option<String>, question: &str) -> Result<String> {
    if let Some(name) = given {
        if !config.profiles.contains_key(&name) {
            return Err(unknown_profile(config, &name));
        }
        return Ok(name);
    }
    if config.profiles.is_empty() {
        bail!("no profile yet, create one with: cprofile add");
    }
    let names: Vec<String> = config.profiles.keys().cloned().collect();
    if !ui.interactive() {
        bail!("missing profile name (available: {})", names.join(", "));
    }
    let options: Vec<String> = names
        .iter()
        .map(|name| {
            let mark = if config.default.as_deref() == Some(name) {
                ui.paint("  default", Style::Dim)
            } else {
                String::new()
            };
            format!("{name}{mark}")
        })
        .collect();
    let index = ui.select(question, &options, &names, None)?;
    // `swap_remove` moves the chosen name out of the list without cloning it.
    let mut names = names;
    Ok(names.swap_remove(index))
}

/// Asks before a change that removes something. `--yes` skips the question;
/// without a terminal there is nobody to ask, so `--yes` is required.
fn confirmed(ui: &Ui, question: &str, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !ui.interactive() {
        bail!("this needs a confirmation: run it again with --yes");
    }
    ui.confirm(question, false)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// `cprofile` with no argument: where you stand, then every command.
fn overview(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        // Do not send the user back to a step that is already done.
        let shim_ready = launch::claude_in_path()
            .first()
            .is_some_and(|(_, is_shim)| *is_shim);
        let next = if shim_ready {
            "cprofile add"
        } else {
            "cprofile setup"
        };
        println!("No profile yet. Start with: {}", ui.command(next));
    } else {
        status_block(ui, env, &loaded, None)?;
        println!();
        profiles_table(ui, env, &loaded)?;
    }
    println!();
    // The same text as `cprofile --help`, built by clap from the `Cli` type.
    // `CommandFactory` is the trait that gives access to it.
    <Cli as clap::CommandFactory>::command()
        .print_help()
        .context("cannot print the help")?;
    Ok(0)
}

/// The config file of the user's shell, when we know how to write to it.
fn shell_config(env: &Env) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").ok()?;
    let name = Path::new(&shell).file_name()?.to_str()?.to_string();
    match name.as_str() {
        "zsh" => Some(env.home.join(".zshrc")),
        // Terminal on macOS starts bash as a login shell, which reads
        // ~/.bash_profile and not ~/.bashrc.
        "bash" if cfg!(target_os = "macos") => Some(env.home.join(".bash_profile")),
        "bash" => Some(env.home.join(".bashrc")),
        _ => None,
    }
}

fn setup(ui: &Ui, env: &Env) -> Result<i32> {
    let bin = env.bin_dir();
    fs::create_dir_all(&bin).with_context(|| format!("cannot create {}", bin.display()))?;
    fs::create_dir_all(env.profiles_dir())?;
    let me = std::env::current_exe()?.canonicalize()?;

    #[cfg(unix)]
    let link = {
        let link = bin.join("claude");
        match fs::remove_file(&link) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("cannot replace {}", link.display()))
            }
        }
        std::os::unix::fs::symlink(&me, &link)
            .with_context(|| format!("cannot create {}", link.display()))?;
        link
    };
    #[cfg(not(unix))]
    let link = {
        let link = bin.join("claude.exe");
        fs::copy(&me, &link).with_context(|| format!("cannot create {}", link.display()))?;
        link
    };

    ui.success(&format!("Shim installed: {}", short_path(env, &link)));
    setup_path(ui, env, &bin)?;

    println!();
    ui.title("Next steps");
    ui.commands(&[
        ("cprofile add", "create a profile, step by step"),
        (
            "cprofile rule add",
            "send a repository or a folder to a profile",
        ),
        ("cprofile doctor", "check the installation"),
    ]);
    Ok(0)
}

/// Makes sure the shim folder is in PATH, or says how to put it there.
fn setup_path(ui: &Ui, env: &Env, bin: &Path) -> Result<()> {
    let in_path = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir == bin));
    if in_path {
        ui.success("The shim folder is in your PATH");
        return Ok(());
    }

    // Written with $HOME when possible, so the line still works if the home
    // folder moves (a restored backup, a renamed account).
    let absolute = bin.display().to_string();
    let with_home = bin
        .strip_prefix(&env.home)
        .ok()
        .map(|rest| format!("$HOME/{}", rest.display()));
    let line = format!(
        "export PATH=\"{}:$PATH\"",
        with_home.as_deref().unwrap_or(&absolute)
    );
    let rc = shell_config(env);

    // Already written by an earlier setup: only a new terminal is missing.
    // We look for both spellings of the folder, absolute and with $HOME.
    if let Some(rc) = &rc {
        let text = fs::read_to_string(rc).unwrap_or_default();
        let mentioned = text.contains(&absolute)
            || with_home
                .as_deref()
                .is_some_and(|spelling| text.contains(spelling));
        if mentioned {
            ui.success(&format!(
                "{} already puts the shim in your PATH",
                short_path(env, rc)
            ));
            ui.detail("open a new terminal for it to apply");
            return Ok(());
        }
    }

    if let Some(rc) = &rc {
        let question = format!("Add the shim to your PATH in {}?", short_path(env, rc));
        if ui.interactive() && ui.confirm(&question, true)? {
            // `append(true)` adds at the end and never rewrites what is there.
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(rc)
                .with_context(|| format!("cannot open {}", rc.display()))?;
            writeln!(
                file,
                "\n# cprofile: the claude shim must come before the real claude\n{line}"
            )
            .with_context(|| format!("cannot write {}", rc.display()))?;
            ui.success(&format!("Added to {}", short_path(env, rc)));
            ui.detail("open a new terminal for it to apply");
            return Ok(());
        }
    }

    ui.warning("The shim folder is not in your PATH yet");
    println!();
    println!("Add this line to your shell config, then open a new terminal:");
    println!();
    println!("  {}", ui.command(&line));
    Ok(())
}

fn add(
    ui: &Ui,
    env: &Env,
    name: Option<String>,
    mut native: bool,
    dir: Option<String>,
    keep_days: Option<u64>,
) -> Result<i32> {
    let mut config = config::load(env)?.config;
    // `map(str::to_string)` copies the name, so `config` is free to change
    // further down while we still hold it.
    let native_owner = config::native_profile(&config).map(str::to_string);

    // No name given: guided mode. The questions only cover what the flags
    // did not say, so `cprofile add work --native` never asks anything.
    let guided = name.is_none();
    let name = match name {
        Some(name) => name,
        None if ui.interactive() => ui.input("Profile name (for example: work, perso):")?,
        None => bail!("missing profile name, for example: cprofile add work"),
    };
    config::validate_name(&name)?;
    if config.profiles.contains_key(&name) {
        bail!(
            "profile '{name}' already exists; start Claude Code in it with: cprofile run -p {name}"
        );
    }

    if guided && !native && dir.is_none() && native_owner.is_none() {
        // Only offered while no profile owns ~/.claude yet.
        native = ui.confirm(
            "Reuse your current Claude Code login (~/.claude) for this profile?",
            config.profiles.is_empty(),
        )?;
    }
    if native {
        if let Some(other) = native_owner {
            bail!(
                "profile '{other}' is already the native one; only one profile can reuse ~/.claude"
            );
        }
    }

    let first = config::add_profile(env, &mut config, &name, native, dir)?;

    ui.success(&format!(
        "Profile '{name}' created{}",
        if first { " and set as default" } else { "" }
    ));
    ui.detail(&location(env, &config, &name)?);

    // Claude Code deletes conversations after 30 days unless told otherwise,
    // and each profile has its own setting: say so now, not after a loss.
    let keep_days = match keep_days {
        Some(days) => Some(days),
        None if guided && ui.interactive() => ask_retention(ui, env, &config, &name)?,
        None => None,
    };
    match keep_days {
        Some(days) => {
            retention::set(env, &config, &name, days)?;
            ui.detail(&format!(
                "keeps its conversations {}",
                retention::days_text(days)
            ));
        }
        None => {
            let current = retention::read(env, &config, &name)?;
            ui.detail(&format!(
                "keeps its conversations {}; change it with: cprofile retention {name}",
                current.describe()
            ));
        }
    }
    if native {
        return Ok(0);
    }

    let login = format!("cprofile run --profile {name}");
    if ui.interactive() && ui.confirm("Start Claude Code in this profile now, to log in?", true)? {
        ui.detail("type /login in Claude Code if it does not ask by itself");
        // On macOS and Linux this call does not return: the process becomes
        // Claude Code. The config is read again to include the new profile.
        let loaded = config::load(env)?;
        return run_claude(env, &loaded, Some(name), Vec::new());
    }
    println!(
        "Log in to its account with: {}   then /login",
        ui.command(&login)
    );
    Ok(0)
}

fn profiles_table(ui: &Ui, env: &Env, loaded: &Loaded) -> Result<()> {
    let config = &loaded.config;
    // Which profile applies in this folder. A broken rule must not hide the
    // list, so an error here just leaves the column empty.
    let here = std::env::current_dir()
        .ok()
        .and_then(|cwd| resolve::resolve(env, loaded, &cwd, None).ok().flatten())
        .map(|found| found.profile);

    let mut rows = Vec::new();
    for name in config.profiles.keys() {
        let is_default = config.default.as_deref() == Some(name);
        let is_here = here.as_deref() == Some(name);
        let rules = loaded
            .rules()
            .filter(|(rule, _)| rule.profile == *name)
            .count();
        rows.push(vec![
            Cell::styled(name, if is_here { Style::Green } else { Style::Plain }),
            account_cell(env, config, name),
            Cell::new(location(env, config, name)?),
            Cell::new(if rules == 0 {
                String::new()
            } else {
                rules.to_string()
            }),
            Cell::new(if is_default { "●" } else { "" }),
            Cell::styled(if is_here { "✓" } else { "" }, Style::Green),
        ]);
    }
    ui.table(
        &[
            "Profile",
            "Account",
            "Config folder",
            "Rules",
            "Default",
            "Here",
        ],
        &rows,
    );
    Ok(())
}

fn list(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        println!(
            "No profile yet. Create one with: {}",
            ui.command("cprofile add")
        );
        return Ok(0);
    }
    profiles_table(ui, env, &loaded)?;
    ui.detail("Default: used when no rule applies.   Here: used in this folder.");
    Ok(0)
}

/// Asks how long a profile should keep its conversations. `None` means
/// "leave it as it is".
fn ask_retention(ui: &Ui, env: &Env, config: &Config, name: &str) -> Result<Option<u64>> {
    let current = retention::read(env, config, name)?;
    let options = vec![
        format!("Leave it: {}", current.describe()),
        "90 days".to_string(),
        "1 year".to_string(),
        "Another number of days".to_string(),
    ];
    let picked = ui.select(
        "Claude Code deletes old conversations. How long should this profile keep them?",
        &options,
        &[],
        Some(0),
    )?;
    Ok(match picked {
        0 => None,
        1 => Some(90),
        2 => Some(365),
        // Asked again until it is a whole number of days, 1 or more.
        _ => loop {
            match ui.input("Number of days:")?.parse::<u64>() {
                Ok(days) if days >= 1 => break Some(days),
                _ => println!("Type a whole number of days, 1 or more."),
            }
        },
    })
}

/// `cprofile retention`: how long each profile keeps its conversations.
fn retention_command(ui: &Ui, env: &Env, name: Option<String>, days: Option<u64>) -> Result<i32> {
    let loaded = config::load(env)?;
    let config = &loaded.config;
    let Some(name) = name else {
        if config.profiles.is_empty() {
            println!(
                "No profile yet. Create one with: {}",
                ui.command("cprofile add")
            );
            return Ok(0);
        }
        let mut rows = Vec::new();
        for name in config.profiles.keys() {
            let kept = retention::read(env, config, name)?;
            let style = match kept {
                Retention::Days(_) => Style::Plain,
                _ => Style::Yellow,
            };
            rows.push(vec![Cell::new(name), Cell::styled(kept.describe(), style)]);
        }
        ui.table(&["Profile", "Keeps conversations"], &rows);
        ui.detail("Older conversations are deleted by Claude Code, without a message.");
        ui.detail(&format!(
            "Change it with: {}   (a project or company settings file can still override it)",
            ui.command("cprofile retention <profile> <days>")
        ));
        return Ok(0);
    };
    if !config.profiles.contains_key(&name) {
        bail!("unknown profile '{name}'");
    }
    let days = match days {
        Some(days) => days,
        None if ui.interactive() => match ask_retention(ui, env, config, &name)? {
            Some(days) => days,
            None => {
                println!("Nothing changed.");
                return Ok(0);
            }
        },
        None => bail!("missing number of days, for example: cprofile retention {name} 365"),
    };
    retention::set(env, config, &name, days)?;
    ui.success(&format!(
        "Profile '{name}' keeps its conversations {}",
        retention::days_text(days)
    ));
    Ok(0)
}

/// `cprofile disk`: the space each profile takes, and what takes it.
fn disk_usage(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        println!(
            "No profile yet. Create one with: {}",
            ui.command("cprofile add")
        );
        return Ok(0);
    }
    let mut rows = Vec::new();
    let mut all = 0;
    for name in loaded.config.profiles.keys() {
        let usage = disk::profile_usage(env, &loaded.config, name)?;
        all += usage.total();
        rows.push(vec![
            Cell::new(name),
            Cell::styled(disk::human(usage.total()), Style::Bold),
            Cell::new(disk::human(usage.sessions)),
            Cell::new(disk::human(usage.file_history)),
            Cell::new(disk::human(usage.extensions)),
            Cell::new(disk::human(usage.other)),
            Cell::styled(location(env, &loaded.config, name)?, Style::Dim),
        ]);
    }
    ui.table(
        &[
            "Profile",
            "Total",
            "Sessions",
            "File history",
            "Plugins, skills",
            "Other",
            "Folder",
        ],
        &rows,
    );
    if rows.len() > 1 {
        ui.detail(&format!("All profiles: {}", disk::human(all)));
    }
    ui.detail("Sessions: your conversations, in projects/. `claude --resume` needs them.");
    ui.detail("Nothing is deleted by this command.");
    Ok(0)
}

fn remove(ui: &Ui, env: &Env, name: Option<String>, purge: bool, yes: bool) -> Result<i32> {
    let loaded = config::load(env)?;
    let name = pick_profile(
        ui,
        &loaded.config,
        name,
        "Which profile do you want to remove?",
    )?;
    let kept = location(env, &loaded.config, &name)?;
    let own_rules = loaded
        .config
        .rules
        .iter()
        .filter(|rule| rule.profile == name)
        .count();
    let team_rules = loaded
        .team_rules
        .iter()
        .filter(|rule| rule.profile == name)
        .count();

    if own_rules > 0 {
        ui.warning(&format!(
            "{own_rules} of your rules point to '{name}' and will be removed too"
        ));
    }
    if team_rules > 0 {
        ui.warning(&format!(
            "{team_rules} team rule(s) need '{name}': claude will refuse to start where they match"
        ));
    }
    // Checked before the question, so a refusal comes first.
    if purge {
        purge::folder(env, &loaded.config, &name)?;
        ui.warning(&format!(
            "{kept} will be deleted with its history, settings and login. This cannot be undone"
        ));
    }
    let question = if purge {
        format!("Remove profile '{name}' and delete its folder?")
    } else {
        format!("Remove profile '{name}'?")
    };
    if !confirmed(ui, &question, yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }

    // `loaded` is no longer needed as a whole, so we take its config out
    // (a move) instead of copying it.
    let mut config = loaded.config;
    let was_default = config.default.as_deref() == Some(name.as_str());
    if purge {
        let purged = purge::purge_profile(env, &mut config, &name)?;
        ui.success(&format!("Profile '{name}' removed"));
        ui.detail(&format!("deleted its Claude Code folder: {kept}"));
        if purged.login {
            ui.detail("removed its login from the keychain");
        }
    } else {
        config::remove_profile(env, &mut config, &name)?;
        ui.success(&format!("Profile '{name}' removed"));
        // The folder holds the history and settings of that account.
        // Deleting it cannot be undone, so it only happens on request.
        ui.detail(&format!("its Claude Code folder is kept: {kept}"));
        ui.detail("to delete it too next time: cprofile remove --purge");
    }
    if was_default {
        ui.warning(&format!(
            "It was the default profile. Choose another with: {}",
            ui.command("cprofile default")
        ));
    }
    Ok(0)
}

fn set_default(ui: &Ui, env: &Env, name: Option<String>) -> Result<i32> {
    let mut config = config::load(env)?.config;
    let name = pick_profile(ui, &config, name, "Which profile when no rule applies?")?;
    config.default = Some(name.clone());
    config::save(env, &config)?;
    ui.success(&format!("Default profile is now '{name}'"));
    Ok(0)
}

fn pin(ui: &Ui, env: &Env, name: Option<String>) -> Result<i32> {
    let loaded = config::load(env)?;
    let name = pick_profile(ui, &loaded.config, name, "Which profile for this folder?")?;
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let file = cwd.join(REPO_FILE);
    fs::write(&file, format!("{name}\n"))
        .with_context(|| format!("cannot write {}", file.display()))?;
    ui.success(&format!("This folder now uses '{name}'"));
    ui.detail(&format!("written to {}", short_path(env, &file)));

    // A locked rule beats the file we just wrote. Say so now, rather than
    // letting the user find out at the next launch.
    if let Ok(Some(found)) = resolve::resolve(env, &loaded, &cwd, None) {
        if found.profile != name {
            ui.warning(&format!(
                "claude will still use '{}' here: {}",
                found.profile,
                found.reason.describe()
            ));
        }
    }
    Ok(0)
}

fn status_block(ui: &Ui, env: &Env, loaded: &Loaded, explicit: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let claude = match launch::find_real_claude(env, &loaded.config) {
        Ok(real) => short_path(env, &real),
        Err(error) => ui.paint(&format!("not found ({error})"), Style::Red),
    };
    match resolve::resolve(env, loaded, &cwd, explicit)? {
        Some(found) => ui.fields(&[
            ("Profile", ui.paint(&found.profile, Style::Green)),
            (
                "Account",
                match account(env, &loaded.config, &found.profile) {
                    Some(email) => email,
                    None => ui.paint("not logged in", Style::Yellow),
                },
            ),
            ("Why", found.reason.describe()),
            ("Config", location(env, &loaded.config, &found.profile)?),
            ("Claude", claude),
        ]),
        None => ui.fields(&[
            (
                "Profile",
                "none (claude runs as if cprofile was not installed)".to_string(),
            ),
            ("Claude", claude),
        ]),
    }
    // Inside a session the shim does not resolve again, so what is printed
    // above is not what a nested `claude` would use.
    if let Ok(session) = std::env::var(launch::SESSION_ENV) {
        ui.warning(&format!(
            "You are inside a Claude Code session started with '{session}': claude keeps that account here"
        ));
    }
    Ok(())
}

fn status(ui: &Ui, env: &Env, explicit: Option<String>) -> Result<i32> {
    let loaded = config::load(env)?;
    status_block(ui, env, &loaded, explicit.as_deref())?;
    Ok(0)
}

/// Builds a rule condition from the current folder, by asking.
/// Returns `(path, git_remote)`.
fn ask_condition(ui: &Ui, env: &Env) -> Result<(Option<String>, Option<String>)> {
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let folder = portable_path(env, ".")?;

    // Each choice: what the user reads, and the condition it stands for.
    let mut choices: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    if let Some(remote) = git::remotes(&cwd).into_iter().next() {
        // `github.com/acme/app` gives the owner pattern `github.com/acme/*`.
        if let Some((owner, _)) = remote.rsplit_once('/') {
            let pattern = format!("{owner}/*");
            choices.push((
                format!("Every repository of {owner}  (git remote {pattern})"),
                None,
                Some(pattern),
            ));
        }
        choices.push((
            format!("Only this repository  (git remote {remote})"),
            None,
            Some(remote),
        ));
    }
    choices.push((
        format!("This folder and everything below  ({folder})"),
        Some(folder),
        None,
    ));

    let options: Vec<String> = choices.iter().map(|(label, _, _)| label.clone()).collect();
    let index = ui.select("Where should the rule apply?", &options, &[], Some(0))?;
    let (_, path, git_remote) = choices.swap_remove(index);
    Ok((path, git_remote))
}

fn rule_add(
    ui: &Ui,
    env: &Env,
    path: Option<String>,
    git_remote: Option<String>,
    profile: Option<String>,
    mut locked: bool,
) -> Result<i32> {
    let mut config = config::load(env)?.config;

    // No condition given: guided mode, built from the current folder.
    let guided = path.is_none() && git_remote.is_none();
    let (path, git_remote) = if !guided {
        let path = path.map(|path| portable_path(env, &path)).transpose()?;
        (path, git_remote)
    } else if ui.interactive() {
        if config.profiles.is_empty() {
            bail!("no profile yet, create one with: cprofile add");
        }
        ask_condition(ui, env)?
    } else {
        bail!(
            "a rule needs --path, --git-remote, or both, for example: \
             cprofile rule add --git-remote \"github.com/acme/*\" --profile work"
        );
    };

    let profile = pick_profile(ui, &config, profile, "Which profile should it use?")?;
    if guided && !locked {
        locked = ui.confirm(
            "Lock it? A locked rule refuses any other profile there.",
            false,
        )?;
    }

    let rule = Rule {
        path,
        git_remote,
        profile,
        locked,
    };
    let summary = format!(
        "{} → {}{}",
        when(&rule),
        rule.profile,
        if locked { " (locked)" } else { "" }
    );
    config.rules.push(rule);
    config::save(env, &config)?;
    ui.success(&format!("Rule added: {summary}"));
    Ok(0)
}

/// Stores a path rule in a form that survives a different working folder:
/// relative paths become absolute, and the home folder is written as `~`.
fn portable_path(env: &Env, path: &str) -> Result<String> {
    if path.starts_with('~') {
        return Ok(path.to_string());
    }
    let absolute = std::env::current_dir()?.join(path);
    let absolute = absolute.canonicalize().unwrap_or(absolute);
    let home = env.home.canonicalize().unwrap_or_else(|_| env.home.clone());
    Ok(match absolute.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => absolute.display().to_string(),
    })
}

fn rule_list(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    let rows: Vec<Vec<Cell>> = loaded
        .rules()
        .enumerate()
        .map(|(index, (rule, source))| {
            vec![
                Cell::new((index + 1).to_string()),
                Cell::new(when(rule)),
                Cell::new(&rule.profile),
                Cell::styled(if rule.locked { "locked" } else { "" }, Style::Yellow),
                Cell::styled(source.label(), Style::Dim),
            ]
        })
        .collect();
    if rows.is_empty() {
        println!(
            "No rule yet. Create one from this folder with: {}",
            ui.command("cprofile rule add")
        );
        return Ok(0);
    }
    ui.table(&["#", "When", "Profile", "Lock", "From"], &rows);
    ui.detail("Checked from top to bottom. A locked rule wins over everything.");
    Ok(0)
}

fn rule_remove(ui: &Ui, env: &Env, number: Option<usize>, yes: bool) -> Result<i32> {
    let loaded = config::load(env)?;
    // Numbers follow `rule list`: team rules come first, then the user's.
    let team = loaded.team_rules.len();
    let mut config = loaded.config;
    if config.rules.is_empty() {
        bail!("you have no rule of your own to remove (see: cprofile rule list)");
    }

    let index = match number {
        Some(number) if number == 0 || number > team + config.rules.len() => {
            bail!("there is no rule {number} (see: cprofile rule list)")
        }
        Some(number) if number <= team => bail!(
            "rule {number} comes from team.toml, which your company provides: it cannot be removed here"
        ),
        Some(number) => number - team - 1,
        None if ui.interactive() => {
            let options: Vec<String> = config
                .rules
                .iter()
                .map(|rule| format!("{} → {}", when(rule), rule.profile))
                .collect();
            ui.select("Which rule do you want to remove?", &options, &[], None)?
        }
        None => bail!("missing rule number (see: cprofile rule list)"),
    };

    let summary = format!(
        "{} → {}",
        when(&config.rules[index]),
        config.rules[index].profile
    );
    if !confirmed(ui, &format!("Remove the rule {summary}?"), yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }
    config.rules.remove(index);
    config::save(env, &config)?;
    ui.success(&format!("Rule removed: {summary}"));
    Ok(0)
}

fn doctor(ui: &Ui, env: &Env) -> Result<i32> {
    let mut failures = 0;
    let mut check = |ok: bool, label: &str, hint: &str| {
        if ok {
            ui.success(label);
        } else {
            failures += 1;
            ui.failure(label);
            ui.detail(hint);
        }
    };

    let loaded = config::load(env);
    check(
        loaded.is_ok(),
        "config files are valid",
        "fix the file named in the error below",
    );
    if let Err(error) = &loaded {
        ui.detail(&format!("{error:#}"));
    }
    let loaded = loaded.unwrap_or_default();

    #[cfg(unix)]
    let shim = env.bin_dir().join("claude");
    #[cfg(not(unix))]
    let shim = env.bin_dir().join("claude.exe");
    check(shim.exists(), "shim is installed", "run: cprofile setup");

    let found = launch::claude_in_path();
    let first_is_shim = found.first().is_some_and(|(_, is_shim)| *is_shim);
    check(
        first_is_shim,
        "`claude` in your PATH is the shim",
        &format!(
            "put {} first in your PATH (cprofile setup can do it), then open a new terminal",
            short_path(env, &env.bin_dir())
        ),
    );

    let real = launch::find_real_claude(env, &loaded.config);
    check(
        real.is_ok(),
        "real Claude Code is found",
        "install Claude Code or set claude_bin in config.toml",
    );
    if let Ok(real) = &real {
        ui.detail(&short_path(env, real));
    }

    check(
        !loaded.config.profiles.is_empty(),
        "at least one profile exists",
        "run: cprofile add",
    );
    let default_ok = match &loaded.config.default {
        Some(name) => loaded.config.profiles.contains_key(name),
        None => true,
    };
    check(
        default_ok,
        "default profile exists",
        "run: cprofile default",
    );
    let rules_ok = loaded
        .rules()
        .all(|(rule, _)| loaded.config.profiles.contains_key(&rule.profile));
    check(
        rules_ok,
        "every rule points to an existing profile",
        "see: cprofile rule list",
    );

    let outside = std::env::var_os(launch::CONFIG_DIR_ENV).is_some()
        && std::env::var_os(launch::SESSION_ENV).is_none();
    check(
        !outside,
        "CLAUDE_CONFIG_DIR is not set by something else",
        "remove it from your shell config: cprofile sets it for each launch",
    );

    // Not a failure: a warning about what Claude Code will delete by itself.
    for name in loaded.config.profiles.keys() {
        match retention::read(env, &loaded.config, name) {
            Ok(Retention::Days(_)) | Err(_) => {}
            Ok(kept) => {
                ui.warning(&format!(
                    "'{name}' keeps its conversations {}",
                    kept.describe()
                ));
                ui.detail(&format!(
                    "older ones are deleted by Claude Code; to keep them: cprofile retention {name} {}",
                    retention::SUGGESTED_DAYS
                ));
            }
        }
    }

    println!();
    if failures == 0 {
        ui.success("Everything is in place");
        Ok(0)
    } else {
        ui.failure(&format!("{failures} problem(s) to fix"));
        Ok(1)
    }
}
