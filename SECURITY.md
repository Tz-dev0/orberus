# Security

Claude Profiles decides which Claude Code account is used in a folder, so a
flaw could send work to the wrong account or weaken a locked rule. Reports
are welcome.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private reporting instead:
on the repository page, open the **Security** tab and choose
**Report a vulnerability**.

Say what you observed, how to reproduce it, and the versions of `cprofile`,
Claude Code and macOS. You will get an answer as soon as the maintainer can
look at it; this is a small project without a guaranteed response time.

## What the tool has access to

- The shim reads `~/.claude-profiles/config.toml`, `team.toml`, and the
  `.git/config` and `.claude-profile` files of the folder you are in. It sets
  one environment variable and starts the real Claude Code. It makes no
  network request.
- `cprofile` reads the account address in each profile's `.claude.json` to
  show it. It does not read tokens or the content of the keychain.
- `cprofile remove --purge` deletes one keychain entry, the login of the
  profile being removed, with the macOS `security` tool.
- `cprofile clean` moves conversation files to the Trash. It does not read
  them beyond their title and working folder.

## Known gaps

- Releases are not signed with an Apple developer certificate. Verify a
  download with the `.sha256` file published next to it.
- A locked rule is a guard against mistakes, not a security boundary: someone
  with access to the account on the Mac can edit the config files or call the
  real Claude Code directly.
