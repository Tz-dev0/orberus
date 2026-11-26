# Commands

`cprofile` alone shows the profile used in the current folder, then the list
of commands. `cprofile <command> --help` describes one command.

Most commands ask for what you leave out when you run them in a terminal: a
numbered list to pick a profile, a yes/no question before removing something.
In a script or a pipe nothing is asked: a missing argument is an error, and
removing something needs `--yes`. Colours follow `NO_COLOR`.

The outputs below are shortened examples.

- Setup: [`setup`](#setup), [`doctor`](#doctor), [`uninstall`](#uninstall)
- Profiles: [`add`](#add), [`list`](#list), [`default`](#default), [`remove`](#remove)
- Choosing a profile: [`status`](#status), [`use`](#use), [`run`](#run), [`rule`](#rule)
- Usage: [`usage`](#usage)
- Disk space and history: [`disk`](#disk), [`retention`](#retention), [`clean`](#clean)

## setup

```sh
cprofile setup
```

Installs the `claude` shim in `~/.claude-profiles/bin` and offers to add that
folder to your `PATH` (zsh and bash). Safe to run again, for instance after
an update.

## doctor

```sh
cprofile doctor
```

Checks the installation and says how to fix what is wrong. It changes nothing.

```text
✓ config files are valid
✓ shim is installed
✓ `claude` in your PATH is the shim
✓ real Claude Code is found
  /opt/homebrew/bin/claude
✓ at least one profile exists
✓ default profile exists
✓ every rule points to an existing profile
✓ CLAUDE_CONFIG_DIR is not set by something else
! 'work' keeps its conversations 30 days (Claude Code default)
  older ones are deleted by Claude Code; to keep them: cprofile retention work 365

✓ Everything is in place
```

Lines that start with `!` are advice, not problems.

## uninstall

```sh
cprofile uninstall [--purge] [--yes]
```

Removes Claude Profiles from this Mac. It lists what it is about to do, then
asks.

```text
$ cprofile uninstall
This removes
  the claude shim: `claude` is the real Claude Code again
  the PATH line that setup added to your shell config
  cprofile itself: ~/.claude-profiles/bin/cprofile
  the menu bar app, moved to the Trash: /Applications/Claude Profiles.app
  kept: your profiles and rules, in ~/.claude-profiles (to delete them too: --purge)
Uninstall Claude Profiles? [y/N]
```

What it removes:

- the `claude` shim, so `claude` is the real Claude Code again;
- the `PATH` line that `setup` wrote in `~/.zshrc`, `~/.bash_profile` or
  `~/.bashrc`. A line you wrote yourself in another way is left alone, and
  you are told about it;
- `cprofile` itself, when it sits in `~/.claude-profiles/bin`. Installed
  another way (`cargo install`), it tells you how to remove it;
- the menu bar app, moved to the Trash.

Your profiles stay: the config, the rules and each profile's Claude Code
folder, with its login and history. Install again later and they are back.

`--purge` deletes those too: `~/.claude-profiles` as a whole, the folder of
every profile, and their logins in the macOS keychain. This cannot be undone.
`~/.claude`, the folder of the native profile, is never touched, and neither
is a profile folder that holds more than the profile.

A `.claude-profile` file left in a project folder does nothing once the tool
is gone.

## add

```sh
cprofile add [name] [--native] [--dir <folder>] [--keep-days <days>]
```

Creates a profile. One profile is one Claude account.

| Option | Effect |
|---|---|
| `--native` | The profile is your existing `~/.claude`: same login, settings and history, left where they are. Only one profile can be native |
| `--dir <folder>` | Store the profile in a folder of your choice instead of `~/.claude-profiles/profiles/<name>` |
| `--keep-days <days>` | Keep its conversations this many days. See [`retention`](#retention) |

The first profile becomes the default. A new profile that is not native
starts empty: log in once with `cprofile run --profile <name>`, then `/login`.

## list

```sh
cprofile list        # or: cprofile ls
```

```text
┌─────────┬─────────────────┬──────────────────────────────────┬───────┬─────────┬──────┐
│ Profile │ Account         │ Config folder                    │ Rules │ Default │ Here │
├─────────┼─────────────────┼──────────────────────────────────┼───────┼─────────┼──────┤
│ perso   │ me@example.com  │ ~/.claude (Claude Code default)  │       │ ●       │ ✓    │
│ work    │ me@acme.example │ ~/.claude-profiles/profiles/work │ 1     │         │      │
└─────────┴─────────────────┴──────────────────────────────────┴───────┴─────────┴──────┘
```

"Account" is the account logged in to the profile, or `not logged in`.
"Here" marks the profile used in the current folder.

## default

```sh
cprofile default [name]        # or: cprofile global
```

Sets the profile used when no rule applies.

## remove

```sh
cprofile remove [name] [--purge] [--yes]        # or: cprofile rm
```

Removes a profile and your rules that point to it. Its Claude Code folder is
kept, so its login and history stay on disk and the profile can be created
again later.

`--purge` also deletes that folder and the profile's login in the macOS
keychain. This cannot be undone. It is refused for the native profile, whose
folder is `~/.claude`.

Team rules are never removed.

## status

```sh
cprofile status
```

Which profile applies in the current folder, and why.

```text
  Profile  work
  Account  me@acme.example
  Why      locked rule git_remote = "github.com/acme/*" in config.toml
  Config   ~/.claude-profiles/profiles/work
  Claude   /opt/homebrew/bin/claude
```

## use

```sh
cprofile use [name]
```

Pins the current folder, and everything below it, to a profile. It writes a
`.claude-profile` file; delete the file to undo.

## run

```sh
cprofile run [--profile <name>] [-- <arguments for claude>]
```

Starts Claude Code. With `--profile`, it forces that profile, unless a locked
rule covers the folder. Typing `claude` does the same as `cprofile run`.

```sh
cprofile run --profile work              # typically to /login the first time
cprofile run --profile work -- --resume  # what follows -- goes to claude
```

`CLAUDE_PROFILE=work claude` is the same explicit choice, for scripts.

## rule

```sh
cprofile rule add [--path <folder>] [--git-remote <pattern>] [--profile <name>] [--locked]
cprofile rule list             # or: cprofile rule ls
cprofile rule remove [number] [--yes]        # or: cprofile rule rm
```

`rule add` with no option builds a rule from the current folder and asks for
the profile. `rule remove` takes the number shown by `rule list` and only
removes your own rules, never team rules.

```text
$ cprofile rule list
┌───┬──────────────────────────────┬─────────┬────────┬─────────────┐
│ # │ When                         │ Profile │ Lock   │ From        │
├───┼──────────────────────────────┼─────────┼────────┼─────────────┤
│ 1 │ git remote github.com/acme/* │ work    │ locked │ config.toml │
└───┴──────────────────────────────┴─────────┴────────┴─────────────┘
  Checked from top to bottom. A locked rule wins over everything.
```

`rule add` refuses a rule for a profile that does not exist, and an exact copy
of a rule you already have.

Rule types and the order of checks are described in [rules.md](rules.md).

## usage

```sh
cprofile usage [profile] [--days N] [--conversations] [--all]
```

Which projects used each account over the last 7 days, and what your rules
say about each one today. It answers a question Claude Code does not: did work
end up on the wrong account? It changes nothing.

```text
perso  me@example.com, last 7 days
┌───────┬─────────────────┬─────────┬───────────────────────────────┐
│ Share │ Project         │ Answers │ Today's rules                 │
├───────┼─────────────────┼─────────┼───────────────────────────────┤
│ 79%   │ ~/code/blog     │ 220     │ no rule, default profile      │
│ 21%   │ ~/code/dotfiles │ 64      │ ✓ pinned with .claude-profile │
└───────┴─────────────────┴─────────┴───────────────────────────────┘
! 79% came from 1 project(s) no rule names: 'perso' is used there only as the default.
  If one belongs to another account, add a rule: cd ~/code/blog && cprofile rule add

work  me@acme.example, last 7 days
┌───────┬─────────────────┬─────────┬─────────────────────────────────────────────────────┐
│ Share │ Project         │ Answers │ Today's rules                                       │
├───────┼─────────────────┼─────────┼─────────────────────────────────────────────────────┤
│ 63%   │ ~/code/acme-api │ 942     │ ✓ locked rule git_remote = "github.com/acme/*"      │
│ 26%   │ ~/code/blog     │ 402     │ would use perso today (default profile)             │
│ 9%    │ ~/code/acme-web │ 148     │ ✓ locked rule git_remote = "github.com/acme/*"      │
│ 2%    │ ~/code/dotfiles │ 31      │ would use perso today (pinned with .claude-profile) │
└───────┴─────────────────┴─────────┴─────────────────────────────────────────────────────┘
! 28% came from 2 project(s) where `claude` would use another profile today.
  They ran with 'work' by an explicit choice, or the rules changed since.
  To keep one on 'work': cd ~/code/blog && cprofile use work
```

The last column reads:

- **✓ rule…** or **✓ pinned**: a rule or a `.claude-profile` file sends this
  project to this profile.
- **no rule, default profile**: nothing names this project. It uses this
  profile only because it is the default.
- **would use X today**: started there now, `claude` would pick another
  profile. The project ran with this one by an explicit choice
  (`cprofile run --profile`, `CLAUDE_PROFILE`), or the rules changed since.
- **folder gone, not checked**: the folder was deleted, so a rule on its git
  remote can no longer be checked.

| Option | Effect |
|---|---|
| `--days N` | Look N days back instead of 7 |
| `--conversations`, `-c` | Show conversations instead of projects (see below) |
| `--all` | List every line, not only the 10 heaviest |

A project is a git repository: sessions started in its subfolders are counted
together. A folder outside any repository counts on its own.

### Conversations that weigh the most

With each answer, Claude reads the whole conversation again. A long
conversation therefore costs more per answer than a short one, whatever you
ask. `--conversations` shows which ones carry the most:

```text
$ cprofile usage work --conversations
work  me@acme.example, last 7 days
┌───────┬─────────┬─────────┬─────────────────┬────────────────────────────┬──────────┐
│ Share │ Context │ Answers │ Project         │ Title                      │ Id       │
├───────┼─────────┼─────────┼─────────────────┼────────────────────────────┼──────────┤
│ 61%   │ 417k    │ 612     │ ~/code/acme-api │ Payment retries            │ 3f2a9c1e │
│ 20%   │ 229k    │ 330     │ ~/code/acme-api │ Migrate the invoices table │ 7b8d0e2f │
│ 16%   │ 142k    │ 402     │ ~/code/blog     │ Blog redesign              │ c0d1e2f3 │
│ 3%    │ 64k     │ 148     │ ~/code/acme-web │ Fix flaky login test       │ a1c4e5f6 │
│ <1%   │ 38k     │ 31      │ ~/code/dotfiles │ (untitled)                 │ e5f6a7b8 │
└───────┴─────────┴─────────┴─────────────────┴────────────────────────────┴──────────┘
  About 88% of this usage went to reading context again, the rest to writing answers.
! 2 conversation(s) send more than 200k tokens with each answer, for 80% of this usage.
  To lighten one: /compact inside it, or a new conversation when the subject changes.
```

**Context** is the number of tokens sent to Claude with each answer, averaged
over the period. Subagents count in the share of their conversation, not in
its context, since each starts with a context of its own. A conversation is
flagged from 200k tokens. An answer copied into several conversations (a
resumed or forked one) counts once, for the conversation written last.

**Share is an estimate.** It is computed from the conversations Claude Code
keeps on this Mac, each answer counted once, with output tokens weighted more
than input and cached tokens less, following the relative API prices. It is
not what your plan counts against its limits, it knows nothing of usage on
claude.ai or on another computer, and conversations already deleted (see
[`retention`](#retention), [`clean`](#clean)) are no longer in it. The format
of these files is internal to Claude Code and may change; lines that cannot be
read are skipped.

## disk

```sh
cprofile disk
```

How much disk space each profile takes, and what takes it. It deletes nothing.

```text
┌─────────┬────────┬──────────┬──────────────┬─────────────────┬────────┐
│ Profile │ Total  │ Sessions │ File history │ Plugins, skills │ Other  │
├─────────┼────────┼──────────┼──────────────┼─────────────────┼────────┤
│ perso   │ 28 MB  │ 28 MB    │ 0 B          │ 0 B             │ 4.1 KB │
│ work    │ 252 MB │ 239 MB   │ 0 B          │ 13 MB           │ 4.1 KB │
└─────────┴────────┴──────────┴──────────────┴─────────────────┴────────┘
  All profiles: 280 MB
  Sessions: your conversations, in projects/. `claude --resume` needs them.
```

Sizes are the space really used on disk, in the same units as the Finder.

## retention

```sh
cprofile retention                    # list every profile
cprofile retention <name> <days>      # set one
```

Claude Code deletes conversations after 30 days by default, without a
message. This command shows and sets that delay for each profile; the
shortest that Claude Code accepts is 1 day.

```text
$ cprofile retention work 365
✓ Profile 'work' keeps its conversations 1 year
```

The value is written as `cleanupPeriodDays` in the profile's `settings.json`;
the other settings in that file are left as they are. A settings file in a
project, or one managed by your company, can still override it.

## clean

```sh
cprofile clean [name] [--session <id>]... [--all] [--yes]
```

Lists a profile's conversations, biggest first, and moves the ones you pick to
the macOS Trash. Without a pick, nothing changes.

```text
$ cprofile clean work
┌───┬────────┬─────────────┬─────────────────┬────────────────────────────┬──────────┐
│ # │ Size   │ Last used   │ Folder          │ Title                      │ Id       │
├───┼────────┼─────────────┼─────────────────┼────────────────────────────┼──────────┤
│ 1 │ 168 MB │ 4 days ago  │ ~/code/acme-api │ Payment retries            │ 3f2a9c1e │
│ 2 │ 63 MB  │ 2 days ago  │ ~/code/acme-api │ Migrate the invoices table │ 7b8d0e2f │
└───┴────────┴─────────────┴─────────────────┴────────────────────────────┴──────────┘
  2 conversation(s), 231 MB in all
  Nothing was changed.
  To move one to the Trash: cprofile clean work --session <id> --yes
```

In a terminal you pick by number. In a script, name each conversation by the
start of its id:

```text
$ cprofile clean work --session 3f2a --yes
✓ 3f2a9c1e  168 MB moved to the Trash
  The space is freed when you empty the Trash. Each one holds a note to put it back.
```

| Option | Effect |
|---|---|
| `--session <id>` | A conversation to move, by the start of its id. Can be repeated |
| `--all` | List every conversation, not only the 15 biggest |
| `--yes` | Do not ask for confirmation |

A conversation that changed in the last hour is refused: it may still be
open. `memory/` folders are never touched. Once a conversation is in the
Trash, `claude --resume` no longer finds it until you put it back.
