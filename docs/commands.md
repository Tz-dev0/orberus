# Commands

`orberus` alone shows the profile used in the current folder, then the list
of commands. `orberus <command> --help` describes one command.

Most commands ask for what you leave out when you run them in a terminal: a
numbered list to pick a profile, a yes/no question before removing something.
In a script or a pipe nothing is asked: a missing argument is an error, and
removing something needs `--yes`. Colours follow `NO_COLOR`.

The outputs below are shortened examples.

- Setup: [`setup`](#setup), [`ide`](#ide), [`doctor`](#doctor), [`uninstall`](#uninstall)
- Profiles: [`add`](#add), [`list`](#list), [`default`](#default), [`remove`](#remove)
- Choosing a profile: [`status`](#status), [`use`](#use), [`run`](#run), [`rule`](#rule)
- Usage: [`usage`](#usage)
- Disk space and history: [`disk`](#disk), [`retention`](#retention), [`clean`](#clean)

## setup

```sh
orberus setup
```

Installs the `claude` shim in `~/.orberus/bin` and offers to add that
folder to your `PATH` (zsh and bash). Safe to run again, for instance after
an update.

It also installs the wrapper that editor extensions can start,
`~/.orberus/ide/claude-wrapper`. Nothing uses it until you run
[`orberus ide on`](#ide).

## ide

```sh
orberus ide [on|off] [editor]
```

For VS Code, Cursor and the other editors built on VS Code, whose Claude Code
extension starts its own copy of Claude Code and so never meets the shim.
Alone, it lists the editors found and says whether their extension goes
through Orberus:

```text
$ orberus ide
┌─────────┬───────────────────────┬─────────────────────────────────────────────────────────┐
│ Editor  │ Claude Code extension │ Settings                                                │
├─────────┼───────────────────────┼─────────────────────────────────────────────────────────┤
│ Cursor  │ skips Orberus         │ ~/Library/Application Support/Cursor/User/settings.json │
│ VS Code │ skips Orberus         │ ~/Library/Application Support/Code/User/settings.json   │
└─────────┴───────────────────────┴─────────────────────────────────────────────────────────┘

To pick the account by folder there too: orberus ide on
```

`orberus ide on` writes the extension's `claudeCode.claudeProcessWrapper`
setting in each editor's user settings, so the extension starts Orberus'
wrapper, which picks the profile for the folder the editor has open and then
starts the extension's own copy of Claude Code. `orberus ide off` removes the
setting. Both act on every editor found, or on the one you name, as the list
shows it: `orberus ide on "VS Code"`. Reload the editor's window after
either.

An editor where the extension is not installed is listed as `not installed`.
`orberus ide on` leaves it alone, unless you name it.

```text
$ orberus ide on
✓ Cursor: its Claude Code extension now goes through Orberus
✓ VS Code: its Claude Code extension now goes through Orberus
```

Only that one setting is touched; the rest of the file, comments included,
stays as it is. A setting that already names another program is left alone.
JetBrains IDEs need none of this: their plugin runs the `claude` command.
See [editors.md](editors.md).

## doctor

```sh
orberus doctor
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
! the Claude Code extension skips Orberus in VS Code
  it starts its own copy of Claude Code; to pick the account by folder there too: orberus ide on
! 'work' keeps its conversations 30 days (Claude Code default)
  older ones are deleted by Claude Code; to keep them: orberus retention work 365

✓ Everything is in place
```

Lines that start with `!` are advice, not problems.

## uninstall

```sh
orberus uninstall [--purge] [--yes]
```

Removes Orberus from this Mac. It lists what it is about to do, then
asks.

```text
$ orberus uninstall
This removes
  the claude shim: `claude` is the real Claude Code again
  the PATH line that setup added to your shell config
  the setting that sends VS Code's Claude Code extension through Orberus
  orberus itself: ~/.orberus/bin/orberus
  the setting that opens the menu bar app when you log in
  the menu bar app, moved to the Trash: /Applications/Orberus.app
  kept: your profiles and rules, in ~/.orberus (to delete them too: --purge)
Uninstall Orberus? [y/N]
```

What it removes:

- the `claude` shim, so `claude` is the real Claude Code again;
- the `PATH` line that `setup` wrote in `~/.zshrc`, `~/.zprofile`,
  `~/.bash_profile` or `~/.bashrc`. A line you wrote yourself in another way is left alone, and
  you are told about it;
- the setting that `orberus ide on` wrote in your editors, then the wrapper
  it names. Your editors' extension starts Claude Code itself again;
- `orberus` itself, when it sits in `~/.orberus/bin`. Installed
  another way (`cargo install`), it tells you how to remove it;
- the menu bar app, moved to the Trash, and the file that opens it when
  you log in.

Your profiles stay: the config, the rules and each profile's Claude Code
folder, with its login and history. Install again later and they are back.

`--purge` deletes those too: `~/.orberus` as a whole, the folder of
every profile, and their logins in the macOS keychain. This cannot be undone.
`~/.claude`, the folder of the native profile, is never touched, and neither
is a profile folder that holds more than the profile.

A `.orberus-profile` file left in a project folder does nothing once the tool
is gone.

## add

```sh
orberus add [name] [--native] [--dir <folder>] [--keep-days <days>]
```

Creates a profile. One profile is one Claude account.

| Option | Effect |
|---|---|
| `--native` | The profile is your existing `~/.claude`: same login, settings and history, left where they are. Only one profile can be native |
| `--dir <folder>` | Store the profile in a folder of your choice instead of `~/.orberus/profiles/<name>` |
| `--keep-days <days>` | Keep its conversations this many days. See [`retention`](#retention) |

The first profile becomes the default. A new profile that is not native
starts empty: log in once with `orberus run --profile <name>`, then `/login`.

## list

```sh
orberus list        # or: orberus ls
```

```text
┌─────────┬─────────────────┬────────┬─────────────────────────────────┬───────┬─────────┬──────┐
│ Profile │ Account         │ Plan   │ Config folder                   │ Rules │ Default │ Here │
├─────────┼─────────────────┼────────┼─────────────────────────────────┼───────┼─────────┼──────┤
│ perso   │ me@example.com  │ Max 5x │ ~/.claude (Claude Code default) │       │ ●       │ ✓    │
│ work    │ me@acme.example │ Team   │ ~/.orberus/profiles/work        │ 1     │         │      │
└─────────┴─────────────────┴────────┴─────────────────────────────────┴───────┴─────────┴──────┘
```

"Account" is the account logged in to the profile, or `not logged in`.
"Plan" is that account's subscription, as Claude Code recorded it at login
(`Pro`, `Max 5x`, `Team`…); empty when it is not known.
"Here" marks the profile used in the current folder.

## default

```sh
orberus default [name]        # or: orberus global
```

Sets the profile used when no rule applies.

## remove

```sh
orberus remove [name] [--purge] [--yes]        # or: orberus rm
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
orberus status
```

Which profile applies in the current folder, and why.

```text
  Profile  work
  Account  me@acme.example (Team)
  Why      locked rule git_remote = "github.com/acme/*" in config.toml
  Config   ~/.orberus/profiles/work
  Claude   /opt/homebrew/bin/claude
```

## use

```sh
orberus use [name]
```

Pins the current folder, and everything below it, to a profile. It writes a
`.orberus-profile` file; delete the file to undo.

## run

```sh
orberus run [--profile <name>] [-- <arguments for claude>]
```

Starts Claude Code. With `--profile`, it forces that profile, unless a locked
rule covers the folder. Typing `claude` does the same as `orberus run`.

```sh
orberus run --profile work              # typically to /login the first time
orberus run --profile work -- --resume  # what follows -- goes to claude
```

`ORBERUS_PROFILE=work claude` is the same explicit choice, for scripts.

## rule

```sh
orberus rule add [--path <folder>] [--git-remote <pattern>] [--profile <name>] [--locked]
orberus rule list             # or: orberus rule ls
orberus rule remove [number] [--yes]        # or: orberus rule rm
```

`rule add` with no option builds a rule from the current folder and asks for
the profile. `rule remove` takes the number shown by `rule list` and only
removes your own rules, never team rules.

```text
$ orberus rule list
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
orberus usage [profile] [--days N] [--conversations] [--all]
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
│ 21%   │ ~/code/dotfiles │ 64      │ ✓ pinned with .orberus-profile │
└───────┴─────────────────┴─────────┴───────────────────────────────┘
! 79% came from 1 project(s) no rule names: 'perso' is used there only as the default.
  If one belongs to another account, add a rule: cd ~/code/blog && orberus rule add

work  me@acme.example, last 7 days
┌───────┬─────────────────┬─────────┬─────────────────────────────────────────────────────┐
│ Share │ Project         │ Answers │ Today's rules                                       │
├───────┼─────────────────┼─────────┼─────────────────────────────────────────────────────┤
│ 63%   │ ~/code/acme-api │ 942     │ ✓ locked rule git_remote = "github.com/acme/*"      │
│ 26%   │ ~/code/blog     │ 402     │ would use perso today (default profile)             │
│ 9%    │ ~/code/acme-web │ 148     │ ✓ locked rule git_remote = "github.com/acme/*"      │
│ 2%    │ ~/code/dotfiles │ 31      │ would use perso today (pinned with .orberus-profile) │
└───────┴─────────────────┴─────────┴─────────────────────────────────────────────────────┘
! 28% came from 2 project(s) where `claude` would use another profile today.
  They ran with 'work' by an explicit choice, or the rules changed since.
  To keep one on 'work': cd ~/code/blog && orberus use work
```

The last column reads:

- **✓ rule…** or **✓ pinned**: a rule or a `.orberus-profile` file sends this
  project to this profile.
- **no rule, default profile**: nothing names this project. It uses this
  profile only because it is the default.
- **would use X today**: started there now, `claude` would pick another
  profile. The project ran with this one by an explicit choice
  (`orberus run --profile`, `ORBERUS_PROFILE`), or the rules changed since.
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
$ orberus usage work --conversations
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

## limits

```sh
orberus limits                 # the 5-hour and weekly limits of each account
orberus limits on work         # have Claude Code report those of a profile
orberus limits off work
```

```text
┌─────────┬────────┬────────┬────────┬────────┬─────────────────────────┐
│ Profile │ 5-hour │ Resets │ Weekly │ Resets │ Updated                 │
├─────────┼────────┼────────┼────────┼────────┼─────────────────────────┤
│ perso   │ 16%    │ in 3h  │ 13%    │ in 4d  │ 4m ago, Claude Desktop  │
│ work    │ 3%     │ in 4h  │ 11%    │ in 4d  │ 2h ago, Claude Code     │
└─────────┴────────┴────────┴────────┴────────┴─────────────────────────┘
  The last numbers Claude Code or Claude Desktop noted for each account, not a live reading.
  Rounded to a whole percent: approximate to about 1% when fresh.
```

A limit belongs to an account: work done in Claude Desktop, on claude.ai and
in Claude Code all counts in it. Orberus never asks Anthropic for these
numbers and never reads your login. It reads what two programs already note
on this Mac, and shows whichever spoke last:

- **Claude Desktop**, while it runs, notes the usage of the account it is
  logged in to every few minutes, in
  `~/Library/Application Support/Claude/plan-usage-history.json`. Nothing to
  turn on: a profile logged in to that same account gets these numbers. They
  come without a reset time, and only for that one account.
- **Claude Code** hands the limits to its
  [status line](https://code.claude.com/docs/en/statusline), for accounts with
  a subscription, after each answer in a terminal. `orberus limits on` makes
  `orberus statusline` the status line of the profile, in its
  `settings.json`: the limits are then noted in
  `~/.orberus/limits/<profile>.json` with their reset times, and a line like
  `work · 5h 3% · week 11%` shows under the prompt. This is the source for
  the accounts Claude Desktop is not logged in to.

Good to know:

- **These are not live numbers.** They are the last ones noted, with their
  age and who noted them. With Claude Desktop closed and no answer in a
  terminal, nothing updates.
- **They are approximate, to about one percent.** Each number is rounded to a
  whole percent, so it can differ by a point from what Claude shows, and by
  more when it is old: check the "Updated" column.
- A number from Claude Desktop keeps the reset time Claude Code announced for
  as long as that time is still ahead; otherwise "Resets" is empty. A limit
  whose window has ended since the last number reads 0%, "started over".
- Claude Desktop's file is its own, not a documented interface: if it
  changes, this source goes quiet and the status line remains.
- A status line of your own is kept: it stays the one shown, and
  `orberus limits off` puts your settings back as they were.
- `orberus uninstall` and `orberus remove` take the status line out again.

## disk

```sh
orberus disk
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
orberus retention                    # list every profile
orberus retention <name> <days>      # set one
```

Claude Code deletes conversations after 30 days by default, without a
message. This command shows and sets that delay for each profile; the
shortest that Claude Code accepts is 1 day.

```text
$ orberus retention work 365
✓ Profile 'work' keeps its conversations 1 year
```

The value is written as `cleanupPeriodDays` in the profile's `settings.json`;
the other settings in that file are left as they are. A settings file in a
project, or one managed by your company, can still override it.

## clean

```sh
orberus clean [name] [--session <id>]... [--all] [--yes]
```

Lists a profile's conversations, biggest first, and moves the ones you pick to
the macOS Trash. Without a pick, nothing changes.

```text
$ orberus clean work
┌───┬────────┬─────────────┬─────────────────┬────────────────────────────┬──────────┐
│ # │ Size   │ Last used   │ Folder          │ Title                      │ Id       │
├───┼────────┼─────────────┼─────────────────┼────────────────────────────┼──────────┤
│ 1 │ 168 MB │ 4 days ago  │ ~/code/acme-api │ Payment retries            │ 3f2a9c1e │
│ 2 │ 63 MB  │ 2 days ago  │ ~/code/acme-api │ Migrate the invoices table │ 7b8d0e2f │
└───┴────────┴─────────────┴─────────────────┴────────────────────────────┴──────────┘
  2 conversation(s), 231 MB in all
  Nothing was changed.
  To move one to the Trash: orberus clean work --session <id> --yes
```

In a terminal you pick by number. In a script, name each conversation by the
start of its id:

```text
$ orberus clean work --session 3f2a --yes
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
