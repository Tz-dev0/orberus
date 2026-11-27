# Contributing

Bug reports, ideas and pull requests are welcome.

## Reporting a problem

Run `orberus doctor` first, then [open an issue](https://github.com/Tz-dev0/orberus/issues)
with its output, the output of `orberus status`, and the versions of
`orberus`, Claude Code and macOS. Remove account addresses and private paths.

For a security problem, see [SECURITY.md](SECURITY.md) instead.

## Building

You need [Rust](https://rustup.rs). The menu bar app also needs Node.js.

```sh
cargo build                 # the core and the command
cargo test                  # unit tests, and end-to-end tests with a fake claude

cd app
npm install
npm run dev                 # the app's interface alone, in a browser, with sample data
npm run tauri dev           # the real app
```

To try your build without touching your own setup, point it at a throwaway
folder:

```sh
ORBERUS_HOME=/tmp/orberus-test cargo run -- add work
```

`scripts/smoke.sh <path to orberus>` runs the everyday commands in an empty
folder and prints how each one ends.

## Layout

| Path | Content |
|---|---|
| `core/` | The core, a library with no interface: everything Orberus knows and does. `config.rs` reads and writes the config, `resolve.rs` picks the profile, `launch.rs` finds and starts the real Claude Code, `tool.rs` describes the tools it manages |
| `cli/` | The `orberus` command, and the shim when it is started as `claude`. `main.rs` declares the commands and hands each one over, `commands/` has one file per command or family of commands, `shim.rs` is the shim, `ui.rs` how things are printed |
| `cli/tests/shim.rs` | End-to-end tests, against a fake `claude` |
| `app/src/` | The app's interface, in TypeScript without a framework |
| `app/src-tauri/` | The app's Rust side (Tauri 2). It is kept out of the workspace, so the command does not pull Tauri's dependencies |
| `scripts/` | Packaging and smoke test |
| `docs/` | The documentation |

## Texts of the app

Every word the window shows is in `app/src/i18n.ts`, once per language. The
Rust side of the app sends data (bytes, seconds, shares from 0 to 1, short
codes), never a sentence, so a text is only ever written in that file.

- A new text: add its key to `en` and to every other language. TypeScript
  refuses a build with a missing key.
- A new language: one more object shaped like `en`, and one more line in
  `LANGUAGES`.
- To look at a language in the browser preview: `npm run dev`, then
  `http://localhost:1420/?lang=fr`.

The command line is in English only.

## Before you open a pull request

```sh
cargo fmt
cargo clippy --all-targets
cargo test
```

Run the same three in `app/src-tauri` when you touch the app.

A few rules the project holds to:

- **The shim stays fast.** On the path that starts Claude Code: no other
  process, no network, no heavy dependency.
- **Fail clearly.** When the config is wrong, the shim stops with a message.
  It never starts Claude Code with an account picked at random.
- **Every resolution rule has a test.** Add a test before changing the order
  in which profiles are chosen.
- **Few dependencies.** Say why in the pull request when you add one.
- **Check before you claim.** Do not write that Claude Code behaves in some
  way unless you tried it or the official documentation says so.
- Code, comments and messages are in English.

The command and the app are two faces of the same core. A rule, a check
or a file format goes in `core/`, so both get it; `cli/` and `app/` only
ask the core and show the answer. The core never reads global state: it is
handed an `Env` (home folder, data folder), which is what lets its tests run
against a temporary folder.

The project is for people with several legitimate, paid accounts. Features
that rotate accounts to get around usage limits will not be merged.

## Releasing (maintainers)

Set the version in `Cargo.toml` (once, for the core and the command), `app/src-tauri/Cargo.toml`,
`app/src-tauri/tauri.conf.json` and `app/package.json`, run `cargo build` in
both crates so the lock files follow, commit, then push a tag with the same
number:

```sh
git tag v0.1.2
git push origin v0.1.2
```

The `Release` workflow runs the tests, builds a universal CLI with
`scripts/package.sh` and a universal app with `scripts/package-app.sh`, and
attaches both archives to a new GitHub release.
