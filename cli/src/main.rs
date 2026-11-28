//! `orberus`: the command line tool. When this same binary is started under
//! the name `claude` (through the link created by `orberus setup`), it acts
//! as the shim: it picks the account for the current folder, then hands over
//! to the real Claude Code. Under the name `claude-wrapper`, it does the
//! same for an editor extension that starts its own copy of Claude Code.

mod commands;
mod shim;
mod ui;

use std::ffi::OsString;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use orberus_core::config;
use orberus_core::paths::Env;
use orberus_core::tool;

use commands::clean::clean;
use commands::desktop::desktop_command;
use commands::disk::disk_usage;
use commands::doctor::doctor;
use commands::ide::ide_command;
use commands::limits::{limits_command, statusline};
use commands::pause::{pause_command, resume_command};
use commands::profiles::{add, list, pin, remove, set_default};
use commands::retention::retention_command;
use commands::rules::{rule_add, rule_list, rule_remove};
use commands::setup::setup;
use commands::status::{overview, status};
use commands::uninstall::uninstall_command;
use commands::usage::usage_command;
use commands::Switch;
use shim::{run_tool, shim, wrapper};
use ui::{Style, Ui};

const EXAMPLES: &str = "\
Examples:
  orberus add                 create a profile, step by step
  orberus use                 pick the profile for this folder
  orberus rule add            send a repository or a folder to a profile
  orberus status              which profile applies here, and why

Most commands ask for what is missing when you leave an argument out.";

#[derive(Parser)]
#[command(
    name = "orberus",
    version,
    about = "The right Claude account for each folder",
    after_help = EXAMPLES
)]
struct Cli {
    // `Option`: with no subcommand we show an overview instead of an error.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Clone, Copy, ValueEnum)]
enum RunTool {
    Claude,
    Codex,
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
        /// Also sign out and delete the profile's Claude and Codex data. Cannot be undone
        #[arg(long)]
        purge: bool,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Show how much disk space each profile takes
    Disk,
    /// Show which folders used each account, and whether a rule covers them
    Usage {
        /// Name of the profile. Without it, every profile is shown
        profile: Option<String>,
        /// How many days back to look
        #[arg(long, default_value_t = 7, value_name = "N")]
        days: u64,
        /// Show conversations instead of projects: which ones weigh the
        /// most, and how much each sends again at every answer
        #[arg(long, short)]
        conversations: bool,
        /// List every line, not only the 10 heaviest
        #[arg(long)]
        all: bool,
    },
    /// List a profile's biggest conversations and move some to the Trash
    Clean {
        /// Name of the profile. Asked when left out
        profile: Option<String>,
        /// A conversation to move to the Trash, by the start of its id.
        /// Can be given several times
        #[arg(long = "session", value_name = "ID")]
        sessions: Vec<String>,
        /// List every conversation, not only the 15 biggest
        #[arg(long)]
        all: bool,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Show each account's 5-hour and weekly limits, or turn them on or off
    Limits {
        /// `on` or `off` for one profile. Without it, the limits are listed
        #[arg(value_enum)]
        switch: Option<Switch>,
        /// Name of the profile. Asked when left out
        profile: Option<String>,
    },
    /// The status line Claude Code runs to report the limits
    #[command(hide = true)]
    Statusline,
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
    /// Pin the current folder to a profile (writes a .orberus-profile file)
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
    /// Start Claude Code or Codex CLI, optionally choosing a profile
    Run {
        /// Tool to start
        #[arg(long, value_enum, default_value = "claude")]
        tool: RunTool,
        #[arg(long, short)]
        profile: Option<String>,
        /// Arguments passed to the selected tool, after `--`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Manage the rules that pick a profile automatically
    Rule {
        #[command(subcommand)]
        command: RuleCommand,
    },
    /// Show your editors, or send their Claude Code extension through Orberus
    Ide {
        /// `on` or `off`. Without it, the editors are listed
        #[arg(value_enum)]
        switch: Option<Switch>,
        /// One editor, by the name it is listed under. Without it, every
        /// editor found
        editor: Option<String>,
    },
    /// Switch Claude Desktop to a profile's account, or list them
    ///
    /// One Claude Desktop at a time: the one that is open is closed, then
    /// it is opened again on the profile's account. Each profile keeps its
    /// own sign-in. Without a name, the profiles are listed with the one
    /// Claude Desktop is on.
    Desktop {
        /// Name of the profile to put Claude Desktop on
        name: Option<String>,
        /// Do not ask before closing the Claude Desktop that is open
        #[arg(long, short)]
        yes: bool,
    },
    /// Stop picking accounts for now: claude runs as if Orberus was not
    /// installed, until `orberus resume`
    Pause,
    /// Pick the account by folder again, after `orberus pause`
    Resume,
    /// Check that everything is wired correctly
    Doctor,
    /// Remove Orberus from this Mac. Profiles are kept unless you ask
    Uninstall {
        /// Also delete every profile: config, rules, Claude Code folders
        /// (history, settings) and logins. Never ~/.claude. Cannot be undone
        #[arg(long)]
        purge: bool,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
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
    // Started under a tool's name (the `claude` link made by `setup`), this
    // program is that tool's shim; under the name of a tool's wrapper, the
    // program an editor extension starts in place of its own copy of the
    // tool; under its own name, the command line.
    let as_tool = tool::for_command(&invoked);
    let as_wrapper = tool::for_wrapper(&invoked);
    let as_shim = as_tool.is_some() || as_wrapper.is_some();

    let result = match (as_tool, as_wrapper) {
        (Some(tool), _) => shim(tool, args.collect()),
        (None, Some(tool)) => wrapper(tool, args.collect()),
        (None, None) => cli(),
    };
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(error) => {
            // The shim names itself, since the user typed `claude` (or
            // opened an editor) and would not know where the message comes
            // from otherwise.
            let prefix = if as_shim { "orberus:" } else { "error:" };
            eprintln!("{} {error:#}", ui::paint_stderr(prefix, Style::Red));
            ExitCode::from(2)
        }
    }
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
        Command::Usage {
            profile,
            days,
            conversations,
            all,
        } => usage_command(&ui, &env, profile, days, conversations, all),
        Command::Clean {
            profile,
            sessions,
            all,
            yes,
        } => clean(&ui, &env, profile, sessions, all, yes),
        Command::Limits { switch, profile } => limits_command(&ui, &env, switch, profile),
        Command::Statusline => statusline(),
        Command::Retention { name, days } => retention_command(&ui, &env, name, days),
        Command::Default { name } => set_default(&ui, &env, name),
        Command::Use { name } => pin(&ui, &env, name),
        Command::Status { profile } => status(&ui, &env, profile),
        Command::Run {
            tool,
            profile,
            args,
        } => {
            let tool = match tool {
                RunTool::Claude => &tool::CLAUDE,
                RunTool::Codex => &tool::CODEX,
            };
            let loaded = config::load(&env)?;
            run_tool(tool, &env, &loaded, profile, args)
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
        Command::Ide { switch, editor } => ide_command(&ui, &env, switch, editor),
        Command::Desktop { name, yes } => desktop_command(&ui, &env, name, yes),
        Command::Pause => pause_command(&ui, &env),
        Command::Resume => resume_command(&ui, &env),
        Command::Doctor => doctor(&ui, &env),
        Command::Uninstall { purge, yes } => uninstall_command(&ui, &env, purge, yes),
    }
}
