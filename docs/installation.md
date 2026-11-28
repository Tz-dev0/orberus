# Installation

Orberus has two parts:

- **`orberus`**, the command-line tool. It is all you need.
- **Orberus.app**, an optional menu bar app that shows the same
  profiles and rules.

Both need macOS, on Apple Silicon or Intel, and
[Claude Code](https://claude.com/claude-code) already installed.

## Install the CLI

```sh
mkdir -p ~/.orberus/bin
curl -fsSL https://github.com/Tz-dev0/orberus/releases/latest/download/orberus-cli-macos-universal.tar.gz \
  | tar -xz -C ~/.orberus/bin --strip-components 1 orberus-cli-macos-universal/orberus
~/.orberus/bin/orberus setup
```

What this does:

1. downloads the latest release and puts `orberus` in `~/.orberus/bin`;
2. `setup` creates `~/.orberus/bin/claude`, a link to `orberus`. This
   is the shim that picks the account when you type `claude`;
3. `setup` offers to add that folder to your `PATH`. With zsh, in two files:
   `~/.zshrc`, which a terminal window reads, and `~/.zprofile`, which apps
   and scripts read when they start a shell of their own. With bash, in your
   bash profile. With another shell, add this line to its config yourself:

   ```sh
   export PATH="$HOME/.orberus/bin:$PATH"
   ```

Open a new terminal, then check the result:

```sh
orberus doctor
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
curl -fsSLO https://github.com/Tz-dev0/orberus/releases/latest/download/orberus-cli-macos-universal.tar.gz
curl -fsSLO https://github.com/Tz-dev0/orberus/releases/latest/download/orberus-cli-macos-universal.tar.gz.sha256
shasum -a 256 -c orberus-cli-macos-universal.tar.gz.sha256
```

## Install the menu bar app

Install the CLI first: the app starts Claude Code through its shim.

```sh
curl -fsSL https://github.com/Tz-dev0/orberus/releases/latest/download/orberus-app-macos-universal.tar.gz \
  | tar -xz -C /Applications
open "/Applications/Orberus.app"
```

The app is not signed either, so the same remark about `curl` applies.

From then on the app opens by itself each time you log in, and macOS may tell
you a background item was added. To stop that, turn off "Start Orberus with
my Mac" at the bottom of its window. See [app.md](app.md#opening-it).

## Update

Run the install commands again. For the app, quit it first (right-click its
menu bar icon, then Quit).

`orberus --version` prints the version you have. The menu bar app tells you
when a later release is out: see [app.md](app.md#updates).

## Uninstall

```sh
orberus uninstall
```

It removes the shim, the `PATH` line that `setup` added to your shell config,
the setting that `orberus ide on` wrote in your editors, `orberus` itself and
the menu bar app (asked to quit when it is running, then moved to the Trash,
with the file that opens it when you log in), after showing you the list and
asking. Open a new terminal afterwards: `claude` is the real Claude Code
again.

Your profiles are kept in `~/.orberus`: the config, the rules and the
Claude Code folder of each profile, with its login and history. To delete
them as well, with their logins in the macOS keychain:

```sh
orberus uninstall --purge
```

Your own `~/.claude` is never touched by either command. See
[commands.md](commands.md#uninstall) for the details.

If you installed `orberus` with `cargo install`, finish with
`cargo uninstall orberus`, in that order. `cargo uninstall` only deletes the
command: alone, it leaves the menu bar app, the shim and the `PATH` line
behind, and `claude` no longer picks an account.

## Build from source

You need [Rust](https://rustup.rs). For the app, also Node.js.

```sh
git clone https://github.com/Tz-dev0/orberus.git
cd orberus
cargo install --path cli    # installs orberus in ~/.cargo/bin
orberus setup
```

The app:

```sh
cd app
npm install
npm run tauri build         # builds "Orberus.app"
```

The built app is in `app/src-tauri/target/release/bundle/macos/`.
