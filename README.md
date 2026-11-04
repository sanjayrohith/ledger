# Ledger

Ledger is a privacy-first personal finance app for macOS. It is a local Tauri desktop application: production builds bundle the React interface into the app and do not start a browser, localhost service, Python process, or network client.

## Current status

Ledger now creates its local SQLite database on first launch in macOS Application Support. The empty state intentionally creates no accounts or financial records. The database uses restrictive owner-only permissions where macOS allows them; it is not application-level encrypted.

Ledger supports local CSV/XLSX imports with preview and duplicate checks, transaction review, local rules, monthly budgets, and a real-data overview. It does not use a bank login, cloud sync, telemetry, or remote assets.

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

Ledger makes no network calls at runtime and includes no telemetry, analytics, remote assets, account login, or financial records in the shell. The SQLite database is stored in macOS Application Support with owner-only permissions where supported. It is not application-level encrypted; use FileVault and a secured macOS account.

## Backup, restore, export, and deletion

- In **Settings**, use **Create backup** to write a self-contained local `.sqlite3` backup. Keep it somewhere protected.
- **Restore backup** validates SQLite integrity and Ledger migration metadata before it replaces local data.
- **Export transactions CSV** creates a portable, local CSV of the imported transaction records.
- **Clear personal financial data** permanently removes accounts, imports, transactions, rules, merchants, and budgets only after typing `DELETE`. Built-in categories remain. Create a backup first if you may need recovery.

To remove Ledger fully, quit the app, move the app bundle to Trash, and delete its `com.local.ledger` folder from your macOS Application Support directory. This is irreversible unless you retained a backup.
