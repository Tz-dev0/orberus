# Claude Profiles

The right Claude account for each folder. Automatically.

`cprofile` keeps several Claude Code accounts on one machine (work, personal,
client) and picks the account from the folder you are in. A company repository
always opens with the company account, a side project with yours, and a locked
rule refuses the wrong one.

Not affiliated with Anthropic. Claude is a trademark of Anthropic.

> Status: early prototype. Claude Code only, tested on Linux so far.

## Install

```sh
cargo install --path .
cprofile setup          # installs the shim and offers to add it to your PATH
```

## Quick start

Most commands ask for what you leave out, so you can start with no argument:

```sh
cprofile add            # name it, and reuse your current login if you want
cprofile add work       # second profile; offers to start Claude Code to log in

cd ~/code/acme-app
cprofile rule add       # proposes "every repository of github.com/acme"

cprofile                # which profile applies here, and all your profiles
```

The same without questions, for scripts:

```sh
cprofile add perso --native
cprofile add work
cprofile rule add --git-remote "github.com/acme/*" --profile work --locked
```

From then on, just type `claude`.

## Commands

| Command | What it does |
|---|---|
| `cprofile` | Profile used in this folder, and the list of profiles |
| `cprofile setup` | Install the `claude` shim and put it in your PATH |
| `cprofile add [name]` | Create a profile (`--native` reuses `~/.claude`) |
| `cprofile list`, `ls` | List profiles: account logged in, default, and the one used here |
| `cprofile disk` | Show how much disk space each profile takes, and what takes it. Deletes nothing |
| `cprofile remove [name]`, `rm` | Remove a profile and its rules; its folder is kept. `--purge` also deletes the folder and its login in the keychain |
| `cprofile default [name]`, `global` | Profile used when no rule applies |
| `cprofile use [name]` | Pin the current folder (writes `.claude-profile`) |
| `cprofile status` | Which profile applies here, and why |
| `cprofile run [-p name] [-- args]` | Start Claude Code, optionally forcing a profile |
| `cprofile rule add` | Add a rule, built from the current folder when no option is given |
| `cprofile rule list`, `ls` | Rules, in the order they are checked |
| `cprofile rule remove [n]`, `rm` | Remove one of your rules by its number |
| `cprofile doctor` | Check the installation |

In a script or a pipe nothing is asked: a missing argument is an error, and a
removal needs `--yes`. Colors follow `NO_COLOR`.

## Menu bar app (macOS, early)

A small app that lives in the menu bar. Click its icon to see your profiles
and who is logged in to each, create or remove a profile, start Claude Code
in one of them (you pick the project folder, a terminal window opens there),
or change the default profile. A “Storage” section shows the disk space of
each profile. Removing a profile keeps its Claude Code
folder unless you tick the box to delete it, as `cprofile remove` and
`cprofile remove --purge` do. It reads and writes the same config as
the CLI.

```sh
cd app
npm install
npm run tauri build     # builds "Claude Profiles.app"
npm run tauri dev       # or run it while working on it
```

The built app is in `app/src-tauri/target/release/bundle/macos/`. Rules are
still created with `cprofile`; the app shows them. It does not handle Claude
Desktop yet.

## How it works

`cprofile setup` creates `~/.claude-profiles/bin/claude`, a link to `cprofile`.
With that folder first in your PATH, typing `claude` starts the shim. It finds
the profile for the current folder, sets `CLAUDE_CONFIG_DIR` to that profile's
folder, then replaces itself with the real Claude Code. Your arguments are
passed as they are.

A profile is picked in this order:

1. a **locked rule** (it also refuses an explicit choice of another profile)
2. an **explicit choice**: `cprofile run --profile x`, or `CLAUDE_PROFILE=x claude`
3. a **`.claude-profile` file** in the folder or a parent (`cprofile use x`)
4. the **first matching rule**, team rules before your own
5. the **default profile**

With no config at all, `claude` runs exactly as if nothing was installed.

## Configuration

`~/.claude-profiles/config.toml`:

```toml
default = "perso"

[profiles.perso]
native = true                    # Claude Code's own ~/.claude, left untouched

[profiles.work]                  # stored in ~/.claude-profiles/profiles/work

[[rule]]
git_remote = "github.com/acme/*"
profile = "work"
locked = true

[[rule]]
path = "~/clients/foo"
profile = "work"
```

`~/.claude-profiles/team.toml` has the same `[[rule]]` entries and is meant to
be handed out by a company. Its rules are checked before yours.

Rules by git remote read `.git/config` directly, so they work wherever the
repository is cloned, including worktrees.

## Known limits

- A running session keeps its account. The choice is made at launch.
- IDE extensions that start their own Claude Code binary may not go through
  the shim.
- Claude Desktop is not handled yet.
- The menu bar app is not signed: macOS asks for confirmation the first time
  it is opened.
