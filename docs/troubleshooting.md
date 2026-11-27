# Troubleshooting

Start with:

```sh
orberus doctor
orberus status
```

`doctor` checks the installation and tells you how to fix each problem.
`status` tells you which profile applies in the current folder, and why.

## Common problems

### `claude` does not use the profile I expect

Run `orberus status` in that folder and read the "Why" line.

- **"default profile"**: no rule matched. Check `orberus rule list`. For a
  git remote rule, the pattern is compared with `host/organisation/repository`
  in lower case; see [rules.md](rules.md#by-git-remote).
- **"file …/.orberus-profile"**: the folder, or a parent, was pinned with
  `orberus use`. Delete that file to undo.
- **"locked rule …"**: a locked rule covers the folder, and nothing overrides
  it. If it comes from `team.toml`, it was set by your company.

### The shim is installed, but `claude` is not the shim

`orberus doctor` reports "`claude` in your PATH is the shim" as failed when
another `claude` comes first in your `PATH`. Make sure this line is in your
shell config, after the lines that add other folders, then open a new
terminal:

```sh
export PATH="$HOME/.orberus/bin:$PATH"
```

### "real Claude Code is found" fails

The shim found no other `claude` in your `PATH`. Install Claude Code, or tell
`orberus` where it is, in `~/.orberus/config.toml`:

```toml
claude_bin = "/full/path/to/claude"
```

### A profile shows "not logged in"

A new profile starts empty. Log in once:

```sh
orberus run --profile work
```

then type `/login` in Claude Code.

### `claude` stops with "refused: this folder is locked…"

A locked rule covers the folder and you asked for another profile, with
`--profile` or `ORBERUS_PROFILE`. This is the guard rail working. Drop the
explicit choice, or change the rule if it is yours.

### `doctor` says `CLAUDE_CONFIG_DIR` is set by something else

Your shell config, or another account switcher, sets `CLAUDE_CONFIG_DIR`
itself. The shim sets that variable on its own for each launch, so remove the
other setting to avoid confusion.

### macOS says the program "cannot be verified" or was "not opened"

The download went through a browser. Install with the `curl` commands of
[installation.md](installation.md) instead, or allow the program in System
Settings, under Privacy & Security.

### Old conversations disappeared

Claude Code deletes conversations after 30 days by default. Keep them longer
with `orberus retention <name> <days>`; `orberus doctor` reminds you of the
profiles that are still on the default.

### I cannot find the menu bar icon

On a Mac with a notch, macOS hides icons when the menu bar is full. Open the
app again from the Finder or Spotlight: its window appears.

## Known limits

- **A running session keeps its account.** The choice is made when Claude
  Code starts. Quit and start again in the right folder to change it.
- **IDE extensions** that start their own copy of Claude Code, instead of the
  `claude` in your `PATH`, do not go through the shim. Terminals inside an IDE
  do.
- **Claude Desktop** is not handled.
- **macOS only** for now. `clean` and `remove --purge` rely on the macOS Trash
  and keychain.
- **`setup` writes the `PATH` line for zsh and bash only.** With another
  shell, add it yourself.
- **Editing through commands drops comments** in `config.toml`.
- **`usage` is an estimate.** It reads the conversations Claude Code keeps on
  this Mac, in a format that is not documented and may change. It does not
  show how much of your plan is left.
- **Two different accounts on one Mac**: logins in separate profiles are
  independent, but this was tested with one account logged in to two
  profiles, not yet with two different accounts.
- Neither the CLI nor the app is signed with an Apple developer certificate.

## Still stuck

[Open an issue](https://github.com/Tz-dev0/orberus/issues) with:

- the output of `orberus doctor` and `orberus status`;
- `orberus --version` and `claude --version`;
- your macOS version.

Remove account addresses and private paths before you post.
