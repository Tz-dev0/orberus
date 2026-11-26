# How it works

## One profile, one config folder

Claude Code keeps its settings, its history and the name of the logged-in
account in a config folder: `~/.claude` by default, or the folder named by the
`CLAUDE_CONFIG_DIR` environment variable. Starting Claude Code with another
config folder gives you another, independent login.

A profile is such a folder:

- a regular profile lives in `~/.claude-profiles/profiles/<name>`, or in the
  folder you gave with `--dir`;
- the **native** profile is `~/.claude` itself. For it, `CLAUDE_CONFIG_DIR` is
  left unset rather than pointed at `~/.claude`, because Claude Code does not
  read exactly the same files in the two cases (`~/.claude.json` sits next to
  the folder when the variable is unset). Your existing setup therefore keeps
  working as it did.

## The shim

`cprofile setup` creates `~/.claude-profiles/bin/claude`, a link to the
`cprofile` binary, and that folder goes first in your `PATH`. `cprofile`
behaves as the shim when it is started under the name `claude`.

When you type `claude`:

1. the shim reads `~/.claude-profiles/config.toml` and `team.toml`;
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
cprofile: work (locked rule git_remote = "github.com/acme/*" in config.toml)
```

Set `quiet = true` in `config.toml` to turn that line off.

### Finding the real Claude Code

The shim walks your `PATH` and takes the first `claude` that is not itself.
If Claude Code is installed somewhere unusual, set `claude_bin` in
`config.toml`. `cprofile status` and `cprofile doctor` show the one it found.

### Inside a session

The shim marks the Claude Code process it starts. A `claude` launched from
inside that session, by a script or a subagent for instance, keeps the
session's account instead of choosing again.

### When something is wrong

If the config cannot be read, names a profile that does not exist, or a
locked rule forbids the profile you asked for, the shim prints the reason and
stops. It never starts Claude Code with a fallback account.

With no config file at all, the shim changes nothing: `claude` runs as if
Claude Profiles was not installed.

## Logins

On macOS, Claude Code stores each login in the keychain, under a name that
depends on the config folder. Two profiles therefore hold two separate
logins: logging in to or out of one does not affect the other.

This is how Claude Code 2.1 behaves on the machines it was tried on. It is
not something Anthropic documents, so a future version could change it.

`cprofile list` shows the account of each profile. It reads the address from
the profile's `.claude.json`; when that file is missing or unexpected, the
profile is shown as `not logged in`.

## What Claude Profiles writes

| Where | What |
|---|---|
| `~/.claude-profiles/config.toml` | Your profiles and rules |
| `~/.claude-profiles/bin/` | `cprofile` and the `claude` shim |
| `~/.claude-profiles/profiles/<name>/` | The Claude Code folder of a profile. Claude Code fills it |
| `~/.claude-profiles/run/` | Small launch scripts written by the menu bar app |
| `<profile folder>/settings.json` | Only the `cleanupPeriodDays` key, when you use `cprofile retention` |
| `.claude-profile` in a folder | When you use `cprofile use` |
| Your shell config | One `PATH` line, if you accept it during `setup` |

It reads, and never edits, `~/.claude-profiles/team.toml`.
