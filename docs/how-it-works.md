# How it works

## One profile, one config folder

Claude Code keeps its settings, its history and the name of the logged-in
account in a config folder: `~/.claude` by default, or the folder named by the
`CLAUDE_CONFIG_DIR` environment variable. Starting Claude Code with another
config folder gives you another, independent login.

A profile is such a folder:

- a regular profile lives in `~/.orberus/profiles/<name>`, or in the
  folder you gave with `--dir`;
- the **native** profile is `~/.claude` itself. For it, `CLAUDE_CONFIG_DIR` is
  left unset rather than pointed at `~/.claude`, because Claude Code does not
  read exactly the same files in the two cases (`~/.claude.json` sits next to
  the folder when the variable is unset). Your existing setup therefore keeps
  working as it did.

## The shim

`orberus setup` creates `~/.orberus/bin/claude`, a link to the
`orberus` binary, and that folder goes first in your `PATH`. `orberus`
behaves as the shim when it is started under the name `claude`.

When you type `claude`:

1. the shim reads `~/.orberus/config.toml` and `team.toml`;
2. it finds the profile for the current folder
   ([in this order](rules.md#how-a-profile-is-chosen));
3. it sets `CLAUDE_CONFIG_DIR` to the profile's folder, or unsets it for the
   native profile;
4. it replaces itself with the real Claude Code, with your arguments unchanged.

Because it is a program in your `PATH` and not a shell function, it works the
same in every shell, in IDE terminals and in scripts.

The shim starts no other process and makes no network request, so it adds no
noticeable delay. On a terminal it prints one line that names the profile:

```text
orberus: work (locked rule git_remote = "github.com/acme/*" in config.toml)
```

Set `quiet = true` in `config.toml` to turn that line off.

### Finding the real Claude Code

The shim walks your `PATH` and takes the first `claude` that is not itself.
If Claude Code is installed somewhere unusual, set `claude_bin` in
`config.toml`. `orberus status` and `orberus doctor` show the one it found.

### Inside a session

The shim marks the Claude Code process it starts. A `claude` launched from
inside that session, by a script or a subagent for instance, keeps the
session's account instead of choosing again.

### When something is wrong

If the config cannot be read, names a profile that does not exist, or a
locked rule forbids the profile you asked for, the shim prints the reason and
stops. It never starts Claude Code with a fallback account.

With no config file at all, the shim changes nothing: `claude` runs as if
Orberus was not installed.

## The wrapper, for editor extensions

The Claude Code extension of VS Code, Cursor and the other editors built on
VS Code carries its own copy of Claude Code and starts it directly, without
looking in your `PATH`. The shim never runs there.

The extension has a setting for this, `claudeCode.claudeProcessWrapper`: a
program it starts in place of its copy, with the copy's path as the first
argument. `orberus setup` installs such a program,
`~/.orberus/ide/claude-wrapper`: one more link to the `orberus` binary, which
behaves as the wrapper when it is started under that name. `orberus ide on`
writes the setting in your editors.

The wrapper does what the shim does, with two differences:

- it starts the copy it was handed, not the `claude` found in your `PATH`
  (which it falls back to when the extension has no copy of its own);
- it always chooses from the folder the editor has open. It has no
  "inside a session" shortcut, since only an editor starts it.

It is kept out of `~/.orberus/bin` on purpose: that folder is in your `PATH`,
and a second name starting with `claude` there would get in the way each time
you type `claude`.

JetBrains IDEs do not need it: their plugin runs the `claude` command, which
is the shim. See [editors.md](editors.md).

## Logins

On macOS, Claude Code stores each login in the keychain, under a name that
depends on the config folder. Two profiles therefore hold two separate
logins: logging in to or out of one does not affect the other.

This is how Claude Code 2.1 behaves on the machines it was tried on. It is
not something Anthropic documents, so a future version could change it.

`orberus list` shows the account of each profile. It reads the address from
the profile's `.claude.json`; when that file is missing or unexpected, the
profile is shown as `not logged in`.

## One tool today

Orberus manages Claude Code only. Inside, what names a tool is kept in one
table (`core/src/tool.rs`): the command people type (`claude`), the variable that
points it to a config folder (`CLAUDE_CONFIG_DIR`) and the folder it uses by
default (`~/.claude`). The shim looks at the name it was started under and
finds its tool in that table.

This is room for other tools later, not support for them: reading who is
logged in, the conversations, their size and the keychain entry all read
Claude Code's own files.

## What Orberus writes

| Where | What |
|---|---|
| `~/.orberus/config.toml` | Your profiles and rules |
| `~/.orberus/bin/` | `orberus` and the `claude` shim |
| `~/.orberus/ide/` | The wrapper that editor extensions start |
| `~/.orberus/profiles/<name>/` | The Claude Code folder of a profile. Claude Code fills it |
| `~/.orberus/run/` | Small launch scripts and notes written by the menu bar app: the last folder of each profile, the terminal you chose |
| `~/Library/LaunchAgents/com.github.tzdev0.orberus.plist` | Written by the menu bar app when it starts, so that macOS opens it at login. Its "Start Orberus with my Mac" switch deletes it |
| `<profile folder>/settings.json` | Only the `cleanupPeriodDays` key, when you use `orberus retention` |
| `.orberus-profile` in a folder | When you use `orberus use` |
| Your shell config | One `PATH` line, if you accept it during `setup` |
| Your editors' `settings.json` | One setting, `claudeCode.claudeProcessWrapper`, when you run `orberus ide on` |

It reads, and never edits, `~/.orberus/team.toml`.
