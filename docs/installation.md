# Installation

Claude Profiles has two parts:

- **`cprofile`**, the command-line tool. It is all you need.
- **Claude Profiles.app**, an optional menu bar app that shows the same
  profiles and rules.

Both need macOS, on Apple Silicon or Intel, and
[Claude Code](https://claude.com/claude-code) already installed.

## Install the CLI

```sh
mkdir -p ~/.claude-profiles/bin
curl -fsSL https://github.com/Tz-dev0/claude-profiles/releases/latest/download/cprofile-macos-universal.tar.gz \
  | tar -xz -C ~/.claude-profiles/bin --strip-components 1 cprofile-macos-universal/cprofile
~/.claude-profiles/bin/cprofile setup
```

What this does:

1. downloads the latest release and puts `cprofile` in `~/.claude-profiles/bin`;
2. `setup` creates `~/.claude-profiles/bin/claude`, a link to `cprofile`. This
   is the shim that picks the account when you type `claude`;
3. `setup` offers to add that folder to your `PATH`, in `~/.zshrc` or in your
   bash profile. With another shell, add this line to its config yourself:

   ```sh
   export PATH="$HOME/.claude-profiles/bin:$PATH"
   ```

Open a new terminal, then check the result:

```sh
cprofile doctor
```

Until you create a profile, `claude` behaves exactly as before.

### Why `curl` and not the browser

The binary is not signed with an Apple developer certificate. macOS refuses
to run an unsigned program that a browser downloaded, but not one fetched
with `curl`. If you did download the archive with a browser, either use the
command above instead, or allow the program in System Settings, under
Privacy & Security.

### Check the download

Each release also has a `.sha256` file next to the archive:

```sh
curl -fsSLO https://github.com/Tz-dev0/claude-profiles/releases/latest/download/cprofile-macos-universal.tar.gz
curl -fsSLO https://github.com/Tz-dev0/claude-profiles/releases/latest/download/cprofile-macos-universal.tar.gz.sha256
shasum -a 256 -c cprofile-macos-universal.tar.gz.sha256
```

## Install the menu bar app

Install the CLI first: the app starts Claude Code through its shim.

```sh
curl -fsSL https://github.com/Tz-dev0/claude-profiles/releases/latest/download/Claude-Profiles-macos-universal.tar.gz \
  | tar -xz -C /Applications
open "/Applications/Claude Profiles.app"
```

The app is not signed either, so the same remark about `curl` applies.

## Update

Run the install commands again. For the app, quit it first (right-click its
menu bar icon, then Quit).

`cprofile --version` prints the version you have.

## Uninstall

```sh
rm -rf "/Applications/Claude Profiles.app"   # the app, if you installed it
rm ~/.claude-profiles/bin/claude ~/.claude-profiles/bin/cprofile
```

Then remove the `export PATH="$HOME/.claude-profiles/bin:$PATH"` line from
your shell config. `claude` is the real Claude Code again.

Your profiles are still on disk: `~/.claude-profiles/config.toml` holds the
profiles and rules, and `~/.claude-profiles/profiles/` holds the Claude Code
folder of each profile, with its history. Delete `~/.claude-profiles` to
remove them too. Their logins are stored in the macOS keychain; to remove
those as well, run `cprofile remove <name> --purge` for each profile before
you uninstall. Your own `~/.claude` is never touched by any of this.

## Build from source

You need [Rust](https://rustup.rs). For the app, also Node.js.

```sh
git clone https://github.com/Tz-dev0/claude-profiles.git
cd claude-profiles
cargo install --path .      # installs cprofile in ~/.cargo/bin
cprofile setup
```

The app:

```sh
cd app
npm install
npm run tauri build         # builds "Claude Profiles.app"
```

The built app is in `app/src-tauri/target/release/bundle/macos/`.
