<div align="center">
  <h1>Claude Profiles</h1>
  <p><b>The right Claude Code account for each folder. Automatically.</b></p>
  <p>
    <a href="https://github.com/Tz-dev0/claude-profiles/releases"><img src="https://img.shields.io/github/v/release/Tz-dev0/claude-profiles?label=version&style=flat-square" alt="Version"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="MIT license"></a>
    <img src="https://img.shields.io/badge/macOS-Apple_Silicon_and_Intel-lightgrey?style=flat-square" alt="macOS, Apple Silicon and Intel">
  </p>
</div>

`cprofile` keeps several Claude Code accounts on one Mac (work, personal,
client) and picks the account from the folder you are in. A company repository
always opens with the company account, a side project with yours, and a locked
rule refuses the wrong one.

```text
~/code/acme-api $ claude
cprofile: work (locked rule git_remote = "github.com/acme/*" in config.toml)

~/code/blog $ claude
cprofile: perso (default profile)

~/code/acme-api $ CLAUDE_PROFILE=perso claude
cprofile: refused: this folder is locked to profile 'work' by rule
git_remote = "github.com/acme/*" in config.toml, so profile 'perso' cannot be used here
```

> Not affiliated with Anthropic. Claude is a trademark of Anthropic.
>
> Early version: Claude Code only, developed and tested on macOS.
> It is meant for people who hold several legitimate, paid accounts. It does
> not switch accounts to get around usage limits.

## Features

- **Automatic choice**: you keep typing `claude`. The account comes from a
  rule on the folder or on the git remote, so there is no switch command to
  forget.
- **Guard rail**: a locked rule wins over everything, and Claude Code does not
  start with another account in that folder.
- **Team rules**: a company can hand out a `team.toml`; its rules are checked
  before yours.
- **Separate logins**: each profile has its own Claude Code folder and its own
  login. Your existing `~/.claude` can be one of them, left untouched.
- **Disk space and history**: see what each profile takes, choose how long
  conversations are kept, and move the biggest ones to the Trash.
- **Menu bar app** (optional): the same profiles and rules, one click away.

## Quick start

Claude Profiles needs macOS (Apple Silicon or Intel) and
[Claude Code](https://claude.com/claude-code) already
installed.

**Install**

```sh
mkdir -p ~/.claude-profiles/bin
curl -fsSL https://github.com/Tz-dev0/claude-profiles/releases/latest/download/cprofile-macos-universal.tar.gz \
  | tar -xz -C ~/.claude-profiles/bin --strip-components 1 cprofile-macos-universal/cprofile
~/.claude-profiles/bin/cprofile setup
```

`setup` installs the `claude` shim next to `cprofile` and offers to add that
folder to your `PATH`. Open a new terminal afterwards. Other ways to install,
and how to update or uninstall, are in [docs/installation.md](docs/installation.md).

**Create your profiles**

```sh
cprofile add perso --native   # your current login, ~/.claude, kept as it is
cprofile add work             # a second, empty profile
cprofile run --profile work   # start Claude Code in it once, then /login
```

**Send a repository to a profile**

```sh
cprofile rule add --git-remote "github.com/acme/*" --profile work --locked
```

From then on, just type `claude`.

Most commands ask for what you leave out, so `cprofile add` and
`cprofile rule add` alone also work: they guide you step by step.

## How a profile is chosen

The first of these that applies wins:

1. a **locked rule**. It also refuses an explicit choice of another profile
2. an **explicit choice**: `cprofile run --profile x`, or `CLAUDE_PROFILE=x claude`
3. a **`.claude-profile` file** in the folder or a parent (`cprofile use x`)
4. the **first matching rule**, team rules before your own
5. the **default profile**

`cprofile status` tells you which one applies where you are, and why:

```text
$ cprofile status
  Profile  work
  Account  me@acme.example
  Why      locked rule git_remote = "github.com/acme/*" in config.toml
  Config   ~/.claude-profiles/profiles/work
  Claude   /opt/homebrew/bin/claude
```

With no config at all, `claude` runs exactly as if nothing was installed.
Rules, the config files and team rules are described in
[docs/rules.md](docs/rules.md).

## Commands

| Command | What it does |
|---|---|
| `cprofile` | Profile used in this folder, then the list of commands |
| `cprofile setup` | Install the `claude` shim and put it in your `PATH` |
| `cprofile add [name]` | Create a profile (`--native` reuses `~/.claude`) |
| `cprofile list` | Profiles, the account logged in to each, the default |
| `cprofile status` | Which profile applies here, and why |
| `cprofile use [name]` | Pin the current folder to a profile |
| `cprofile default [name]` | Profile used when no rule applies |
| `cprofile run [-p name] [-- args]` | Start Claude Code, optionally forcing a profile |
| `cprofile rule add`, `list`, `remove` | Manage the rules |
| `cprofile disk` | Disk space of each profile. Deletes nothing |
| `cprofile retention [name] [days]` | How long each profile keeps its conversations |
| `cprofile clean [name]` | Biggest conversations of a profile; move some to the Trash |
| `cprofile remove [name]` | Remove a profile and your rules that point to it |
| `cprofile doctor` | Check the installation |

Every command, with its options and an example of its output, is in
[docs/commands.md](docs/commands.md).

## Features in detail

The examples below are shortened.

### Profiles

```text
$ cprofile list
┌─────────┬─────────────────┬──────────────────────────────────┬───────┬─────────┬──────┐
│ Profile │ Account         │ Config folder                    │ Rules │ Default │ Here │
├─────────┼─────────────────┼──────────────────────────────────┼───────┼─────────┼──────┤
│ perso   │ me@example.com  │ ~/.claude (Claude Code default)  │       │ ●       │ ✓    │
│ work    │ me@acme.example │ ~/.claude-profiles/profiles/work │ 1     │         │      │
└─────────┴─────────────────┴──────────────────────────────────┴───────┴─────────┴──────┘
  Default: used when no rule applies.   Here: used in this folder.
```

A profile is a Claude Code config folder with its own login, settings and
history. The profile created with `--native` is your existing `~/.claude`.

### Disk space

```text
$ cprofile disk
┌─────────┬────────┬──────────┬──────────────┬─────────────────┬────────┐
│ Profile │ Total  │ Sessions │ File history │ Plugins, skills │ Other  │
├─────────┼────────┼──────────┼──────────────┼─────────────────┼────────┤
│ perso   │ 28 MB  │ 28 MB    │ 0 B          │ 0 B             │ 4.1 KB │
│ work    │ 252 MB │ 239 MB   │ 0 B          │ 13 MB           │ 4.1 KB │
└─────────┴────────┴──────────┴──────────────┴─────────────────┴────────┘
  All profiles: 280 MB
  Nothing is deleted by this command.
```

### Cleaning up conversations

`cprofile clean` lists a profile's conversations, biggest first. Nothing moves
until you pick some, and what you pick goes to the macOS Trash with a note
that says where to put it back.

```text
$ cprofile clean work
┌───┬────────┬─────────────┬─────────────────┬────────────────────────────┬──────────┐
│ # │ Size   │ Last used   │ Folder          │ Title                      │ Id       │
├───┼────────┼─────────────┼─────────────────┼────────────────────────────┼──────────┤
│ 1 │ 168 MB │ 4 days ago  │ ~/code/acme-api │ Payment retries            │ 3f2a9c1e │
│ 2 │ 63 MB  │ 2 days ago  │ ~/code/acme-api │ Migrate the invoices table │ 7b8d0e2f │
│ 3 │ 8.4 MB │ 16 days ago │ ~/code/acme-api │ Fix flaky login test       │ a1c4e5f6 │
└───┴────────┴─────────────┴─────────────────┴────────────────────────────┴──────────┘
  3 conversation(s), 239 MB in all
  Nothing was changed.
```

### How long conversations are kept

Claude Code deletes conversations after 30 days by default, without a
message. `cprofile retention` shows the setting of each profile and changes it:

```text
$ cprofile retention work 365
✓ Profile 'work' keeps its conversations 1 year
```

### Menu bar app

A small app that lives in the menu bar: your profiles and who is logged in to
each, the rules, the disk space, and a button to open Claude Code in a profile.
It reads and writes the same config as the CLI and is optional. See
[docs/app.md](docs/app.md).

## Safety

- The shim only chooses a config folder, then hands over to the real Claude
  Code. It does not read your conversations or your credentials, and it makes
  no network request.
- If the config cannot be read, or a locked rule forbids the profile you asked
  for, Claude Code does not start. It never falls back to another account.
- `disk`, `list`, `status` and `doctor` change nothing.
- `clean` moves conversations to the Trash and refuses one that changed in the
  last hour, since it may still be open. It never touches `memory/` folders.
- `remove` keeps the profile's folder, so its login and history stay on disk.
  Only `remove --purge` deletes them, and it is refused for `~/.claude`.

Known limits are listed in [docs/troubleshooting.md](docs/troubleshooting.md#known-limits).
To report a vulnerability, see [SECURITY.md](SECURITY.md).

## Documentation

- [Installation](docs/installation.md): install, update, uninstall, build from source
- [Rules and configuration](docs/rules.md): rule types, `config.toml`, team rules
- [Commands](docs/commands.md): every command and its options
- [Menu bar app](docs/app.md)
- [How it works](docs/how-it-works.md)
- [Troubleshooting](docs/troubleshooting.md): known limits and common problems

## Get help

First run `cprofile doctor` and read what it says: it checks the shim, your
`PATH`, the config files and the rules, and tells you how to fix what is wrong.

Then read [Troubleshooting](docs/troubleshooting.md). If the problem is still
there, [open an issue](https://github.com/Tz-dev0/claude-profiles/issues) with
the output of `cprofile doctor` and `cprofile status`.

## Contributing

Bug reports, ideas and pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) for how to build and test the project.

## License

[MIT](LICENSE).
