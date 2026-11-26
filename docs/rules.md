# Rules and configuration

A rule says "in this place, use this profile". This page describes the rule
types, the order in which everything is checked, and the files behind it.

## How a profile is chosen

When you type `claude`, the first of these that applies wins:

1. a **locked rule** that matches the folder. An explicit choice of another
   profile is then refused, and Claude Code does not start (exit code 2);
2. an **explicit choice**: `cprofile run --profile x`, or the environment
   variable `CLAUDE_PROFILE=x`;
3. a **`.claude-profile` file** in the folder or one of its parents, written
   by `cprofile use x`;
4. the **first rule that matches**, team rules before your own, each list
   from top to bottom;
5. the **default profile**, set by `cprofile default x`.

`cprofile status` shows the result for the current folder and the reason.
`cprofile rule list` shows the rules in the order they are checked.

A `claude` started from inside a running session keeps that session's
account: the choice is made once, at launch.

## Rule types

### By folder

```sh
cprofile rule add --path ~/clients/foo --profile client
```

The rule covers the folder and everything below it. `~` is accepted.

### By git remote

```sh
cprofile rule add --git-remote "github.com/acme/*" --profile work
```

The pattern is compared with the repository's remote, written as
`host/organisation/repository` in lower case, whatever the form of the URL
(`git@github.com:Acme/Api.git` and `https://github.com/acme/api` both become
`github.com/acme/api`). `*` stands for any text.

This kind of rule follows the repository wherever it is cloned. The remote is
read from `.git/config` directly, without running `git`; worktrees and
submodules are handled.

### Both conditions

A rule with `--path` and `--git-remote` matches only when both do.

### Locked rules

```sh
cprofile rule add --git-remote "github.com/acme/*" --profile work --locked
```

A locked rule is checked before everything else, and it refuses any other
profile in the place it covers:

```text
$ CLAUDE_PROFILE=perso claude
cprofile: refused: this folder is locked to profile 'work' by rule
git_remote = "github.com/acme/*" in config.toml, so profile 'perso' cannot be used here
```

### Without options

Run `cprofile rule add` alone inside a repository or a folder: it proposes a
rule built from where you are, and asks for the profile.

## Pinning one folder

```sh
cd ~/code/blog
cprofile use perso
```

This writes a `.claude-profile` file that holds the profile's name. It applies
to the folder and everything below it. To undo it, delete the file. You may
want to add `.claude-profile` to your global gitignore.

## The config file

Everything above is stored in `~/.claude-profiles/config.toml`. You can edit
it by hand; the commands rewrite it without your comments.

```toml
default = "perso"                # profile used when no rule applies

[profiles.perso]
native = true                    # Claude Code's own ~/.claude, left untouched

[profiles.work]                  # stored in ~/.claude-profiles/profiles/work

[profiles.client]
dir = "~/clients/foo/.claude-config"   # a custom folder for this profile

[[rule]]
git_remote = "github.com/acme/*"
profile = "work"
locked = true

[[rule]]
path = "~/clients/foo"
profile = "client"
```

| Key | Meaning |
|---|---|
| `default` | Profile used when nothing else applies |
| `quiet = true` | Do not print the `cprofile: work (…)` line when Claude Code starts |
| `claude_bin = "/path/to/claude"` | Where the real Claude Code is, when it cannot be found in your `PATH` |
| `[profiles.<name>]` | One profile. `native = true` means `~/.claude`; only one profile can be native. `dir` sets a custom folder |
| `[[rule]]` | One rule: `path` and/or `git_remote`, `profile`, and optionally `locked = true` |

An unknown key is an error, so a typo does not go unnoticed. When the config
cannot be read, `claude` stops with a message instead of starting with an
account chosen at random.

## Team rules

`~/.claude-profiles/team.toml` holds `[[rule]]` entries in the same format.
It is meant to be written once by a company and handed out to its developers,
with whatever the company already uses to distribute files.

```toml
[[rule]]
git_remote = "github.com/acme/*"
profile = "work"
locked = true
```

Team rules are checked before your own, and `cprofile` never edits that file:
`cprofile rule remove` and `cprofile remove` only touch your rules. A team
rule names a profile, so every developer needs a profile with that name.

## Environment variables

| Variable | Effect |
|---|---|
| `CLAUDE_PROFILE` | Explicit choice of a profile for one command |
| `CLAUDE_PROFILES_HOME` | Use another folder than `~/.claude-profiles` |
| `NO_COLOR` | Turn colours off |
