# Ledger

Ledger is a privacy-first personal finance app for macOS. It is a local Tauri desktop application: production builds bundle the React interface into the app and do not start a browser, localhost service, Python process, or network client.

## Milestone 0 status

Ledger now creates its local SQLite database on first launch in macOS Application Support. The empty state intentionally creates no accounts or financial records. The database uses restrictive owner-only permissions where macOS allows them; it is not application-level encrypted.

Milestone 2 adds local CSV import. Select a file through the native picker, choose a local account, review the detected mapping and row validation, then explicitly confirm the atomic commit. The importer supports manually confirmed date, narration, debit, credit, amount/direction, balance, and reference columns. It does not yet claim a hard-coded Canara preset or support XLSX; both need confirmed statement-format evidence or the next milestone.

## Requirements

- macOS on Apple Silicon
- Xcode Command Line Tools
- Rust stable (installed through `rustup`)
- Node.js and npm

## Development

```sh
npm install
npm run tauri dev
```

`tauri dev` uses a loopback Vite server only while developing. The production application contains static frontend assets and does not need a server or browser.

## Checks

```sh
npm run typecheck
npm run lint
npm test
cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo build
npm run tauri build
```

The macOS `.app` and `.dmg` build artifacts are produced under `src-tauri/target/release/bundle/`. The initial build is unsigned and not notarised; macOS may require an explicit first-open approval for personal use.

## Privacy

Ledger makes no network calls at runtime and includes no telemetry, analytics, remote assets, account login, or financial records in the shell. Future imports will be explicit and local-only.
