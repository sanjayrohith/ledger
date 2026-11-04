# Ledger — Privacy-First Personal Finance for macOS

**Status:** Implementation-ready product and technical specification  
**Working name:** Ledger (provisional)  
**Platform:** macOS desktop, personal use  
**Data model:** Local-first, offline-capable, single user  
**Target software and service cost:** ₹0 recurring; no paid APIs or hosted services

## 1. Purpose

Ledger helps one person understand spending from locally imported account statements, with a focus on UPI transactions. It should make monthly spending patterns clear, reduce repeated categorisation work, and support realistic planning for the next month.

The primary data source is a Canara Bank transaction statement in CSV or XLSX format. Google Pay and BHIM exports may optionally add readable merchant or transaction details. They are not required for core operation and must never create duplicate spending when the same payment is already present in the bank statement.

Ledger is an import-and-analysis tool. It does not connect to a bank, initiate payments, store banking credentials, or rely on an internet service.

## 2. Non-negotiable product constraints

These requirements take precedence over convenience or visual demos:

1. **Real macOS desktop application.** Build with Tauri 2 and produce a packaged `.app`; provide a `.dmg` for installation when practical. Do not deliver a localhost website, browser-only app, or app that requires the user to start a server or terminal command for normal use.
2. **No runtime localhost server.** The frontend assets are bundled in the Tauri application and rendered in the macOS system WebView. No browser window or separate web server is part of the runtime architecture.
3. **Local-first and offline capable.** Imports, search, categorisation, budgets, charts, backups, and exports work without internet access. No account or cloud sync is required.
4. **Privacy by design.** Financial data, original statement contents, and derived insights stay on the Mac. No telemetry, crash-report upload, analytics SDK, remote fonts, remote assets, or network calls from the app.
5. **Zero recurring cost.** Use free and open-source dependencies and local storage. No paid API, subscription, hosted database, or cloud infrastructure. Code signing/notarisation needed for convenient public distribution may carry an Apple Developer membership cost; this is not required for a self-built personal copy and must be disclosed if distribution expands.
6. **Minimal polished interface.** Dark, calm, premium visual design with clear typography, spacing, restrained accent colour, strong hierarchy, and purposeful charts. Avoid a generic admin template, clutter, gratuitous gradients, excessive animation, and placeholder financial data presented as real.
7. **Fast and resource-conscious.** No Electron/Chromium bundle, Python runtime, background daemon, local AI model, polling loop, or unnecessary UI library. Render large transaction histories efficiently and query SQLite for filtered/paginated results.
8. **Correct financial treatment.** Transfers, refunds, reversals, fees, income, and expenses must not be conflated. Ambiguous rows remain reviewable instead of being silently guessed.
9. **Low debugging burden.** Implement one milestone at a time. Add and run automated tests for each milestone, build/package checks at the stated gates, and stop with a concise blocker report when a key assumption or build prerequisite cannot be resolved safely.
10. **Do not overbuild.** No multi-user support, web service, mobile client, bank API/Account Aggregator integration, PDF parsing, machine-learning classification, or generic plugin framework in the initial release.

## 3. Users and outcomes

### User

One person using a Mac, primarily paying through UPI, who can download Canara Bank statements as CSV or Excel files and wants to understand personal spending without uploading financial data.

### The app should answer

- How much money came in and went out during a selected period?
- How much was actual spending, excluding transfers and correctly accounting for refunds?
- Which categories and merchants account for the most spending?
- How did spending compare with the prior month or another selected period?
- Which imported transactions still need review?
- How much of each monthly category budget remains?

The app reports what the data supports. It must label missing or partial statement coverage and must not imply a complete financial picture if only some accounts or dates were imported.

## 4. Scope

### Initial release (P0)

- First-run local database setup and a useful empty state.
- Add/rename/archive a local account (start with Canara Bank; no credentials).
- Import Canara Bank CSV and XLSX using detected headers plus a user-confirmable column mapping.
- Import preview with row validation, duplicate estimate, totals and date range.
- Atomic import: either the chosen valid batch is committed or none of it is.
- Transactions list with search, date range, account, direction, category, and review filters.
- Manual transaction categorisation and merchant-name correction.
- Starter categories and deterministic user-editable merchant rules.
- Dashboard with current-month income, expense, net cash movement, category breakdown, and prior-month comparison, clearly marked when the period is incomplete.
- Local SQLite storage, migrations, CSV export, and backup/restore of the app database.
- Settings for currency display (default INR), first day of month (default 1), and database/backup location visibility.
- Packaged macOS `.app` and a `.dmg` build artifact.

### Next release (P1)

- Monthly category budgets and remaining-per-day guidance.
- Spending trend and merchant analytics over longer ranges.
- Recurring transaction suggestions, explicitly requiring user review.
- Import of Google Pay and BHIM transaction exports as optional enrichment sources, only after actual sample files and formats are available. No screen scraping or account login.
- XLSX sheet selection and better mapping presets based on confirmed Canara statement samples.
- Encrypted backup export if it can be implemented and tested with a maintained, well-understood library without compromising recovery.

### Explicitly out of scope initially

- Direct bank login, scraping, UPI PIN/OTP collection, payment initiation, or automatic bank synchronisation.
- RBI Account Aggregator or other financial-data-provider integrations.
- PDF statement extraction until CSV/XLSX import is stable.
- iOS/Android clients, browser app, LAN access, or cloud sync.
- AI-generated financial advice or remote AI APIs.
- Tax filing, investment accounting, lending, shared households, or multi-currency accounting.
- Automatic deletion of source statements or hidden import of files.

## 5. Architecture

```text
┌─────────────────────────────────────────────┐
│ Tauri 2 macOS desktop shell                 │
│ Bundled React + TypeScript assets           │
│ System WebView (WKWebView), no local server │
└──────────────────────┬──────────────────────┘
                       │ typed, narrow Tauri commands
┌──────────────────────▼──────────────────────┐
│ Rust application layer                      │
│ Import → normalise → validate → deduplicate │
│ Rules → accounting classification → queries│
└──────────────────────┬──────────────────────┘
                       │ parameterised SQL
┌──────────────────────▼──────────────────────┐
│ SQLite database in macOS Application Support│
│ Migrations, constraints, indexes, backups   │
└─────────────────────────────────────────────┘
```

### Boundary rules

- Rust owns file parsing, validation, deduplication, database access, accounting logic, and aggregation. React is presentation and user interaction only.
- The frontend never opens arbitrary filesystem paths or accesses SQLite directly. Use Tauri's dialog/plugin capabilities to select files and a small set of explicitly registered commands to preview, commit, query, edit, back up, and restore.
- Choose a maintained Rust SQLite library (for example `rusqlite` with bundled SQLite if compatible with current Tauri/macOS build requirements). Use SQLite transactions and prepared/parameterised statements.
- Use a CSV parser and an XLSX reader (for example `csv` and `calamine`) only when needed. Keep dependency count small. Pin compatible versions in lockfiles and avoid adding a framework for a one-off feature.
- Do not create a Python service or subprocess. Do not add a separate HTTP API. Do not add a state-management library until simple component state and typed command wrappers prove insufficient.
- Use a chart library only if it materially reduces complexity and is tree-shaken/imported minimally. A few simple charts can use a light, maintained library or SVG; do not bundle a large general dashboard system by default.
- Use React Router only if multiple navigable screens need it; a small sidebar and a few screens may use a simple internal route/state model.

### Recommended stack

| Layer | Choice | Notes |
|---|---|---|
| Desktop shell | Tauri 2 | Native packaged app, system WebView, capability-based permissions |
| UI | React + TypeScript + Vite | Bundled static assets; strict TypeScript; no runtime server |
| Styling | CSS variables and small component styles; Tailwind optional | Prefer a deliberate design system without a heavy component kit |
| Backend | Rust stable | File parsing, domain rules, SQLite, Tauri commands |
| Database | SQLite | Local; foreign keys, migrations, indexes, WAL if validated |
| CSV/XLSX | Rust `csv` and `calamine` or similarly small maintained crates | No PDF in first release |
| IDs/serialization | UUID/ULID only if useful; `serde` | Use stable identifiers and explicit DTOs |
| Frontend checks | TypeScript compiler, ESLint, Vitest + Testing Library if useful | Unit/component coverage for UI behavior |
| Backend checks | Rust unit/integration tests | Import, accounting, deduplication, migration, query tests |

Do not adopt all libraries from this table blindly. Before adding a dependency, explain the feature it enables, verify maintenance and license, and prefer the smallest option that satisfies the requirement.

## 6. Data import and normalisation

### Statement import flow

1. User explicitly selects a CSV or XLSX file through the native file picker.
2. The app reads the selected file locally. It must not copy it to a cloud service or keep an unnecessary temporary copy.
3. Detect delimiter/encoding for CSV where practical, or show a readable error and mapping preview. For XLSX, let the user choose among worksheets when there is more than one plausible sheet.
4. Inspect headers and propose a mapping. Candidate Canara fields include transaction date, value date, narration/description, withdrawal/debit, deposit/credit, amount, balance, cheque/reference/UTR. These are candidates, not guaranteed headers: confirm them against a redacted real export before hard-coding a bank preset.
5. Preview rows and totals. Show valid rows, invalid rows with row number and reason, likely duplicates, debit/credit totals, selected date range, and mapped columns. Never silently discard invalid rows.
6. Require explicit confirmation before commit. Invalid rows may be excluded only after the user sees the count and reasons; do not partially import without a clear choice.
7. Insert the accepted rows, import-batch metadata, and any safe classifications in one SQLite transaction. On failure, roll back the batch and preserve the existing database.
8. Show a final summary and a path back to the batch/transactions for review.

### Canonical transaction row

Normalize source rows to a canonical shape before persistence:

- transaction date (required; ISO `YYYY-MM-DD`)
- optional value date
- raw description/narration (preserve exactly as imported for auditability)
- optional cleaned merchant/counterparty
- amount in integer minor units (paise), never floating point
- direction: `debit` or `credit`
- optional resulting balance in paise
- optional bank reference/UTR/cheque reference
- source account
- source format and import batch
- transaction kind: `expense`, `income`, `transfer`, `refund`, `reversal`, `fee`, or `unknown`
- category, classification source, review state

The importer must correctly handle statement formats that have separate debit/credit columns, one signed amount column, or a debit/credit indicator. If a row has both debit and credit amounts, reject it for review unless the format explicitly documents a valid interpretation. Reject zero/malformed amounts and unparseable dates with row-specific feedback.

### Optional Google Pay/BHIM enrichment

- Bank rows remain the accounting source of truth.
- UPI app exports may be imported as supplemental evidence only when a real export format is available. Keep provider/source provenance.
- Match a supplemental row to an existing bank transaction only with a transparent deterministic score using available reference, date, amount, direction, and normalized party. Show ambiguous matches for user review.
- An enrichment match updates merchant/description metadata on the existing transaction; it must not add another expense or income entry.
- If unmatched, show it in a separate staging/review list. Do not count it in totals until the user explicitly chooses to import it as a financial transaction.
- Never ask for Google, BHIM, bank, UPI, OTP, or PIN credentials.

## 7. Accounting and categorisation rules

These rules are product behavior and must have automated tests.

1. **Store money as integer paise.** UI formats as INR (₹) by default. Never store or total currency as binary floating-point values.
2. **Direction is explicit.** Debit and credit are separate from transaction kind. Do not infer income solely from positive/negative signs without interpreting the statement's column semantics.
3. **Expense totals.** Include user-confirmed `expense` and `fee` debits, net of linked/appropriately classified refunds. Exclude `transfer`, `unknown`, and unconfirmed staging rows from expense totals. Show unknown totals separately if useful.
4. **Income totals.** Include classified income credits. Refunds and reversals are not income. Transfers are not income.
5. **Transfers.** A transfer between the user's own accounts is not income or expense. Since the first release may use one account, make transfer classification available and avoid claiming cross-account matching until multiple account statements exist. Do not automatically assume every UPI debit is an expense; allow transfer/own-account classification.
6. **Refunds and reversals.** Record as credit-direction rows with `refund` or `reversal` kind and optionally link to the original transaction. Reduce net expense only when the association/classification is explicit or sufficiently supported and reviewed. Do not delete the original debit.
7. **Failed/reversed payments.** A reversal credit paired to a debit nets the event without counting the reversal as income. Preserve both statement rows.
8. **Opening balance and balance rows.** Balances are reconciliation metadata, not transactions. Never include them in income/expense.
9. **Duplicate imports.** Prefer a source reference unique within an account where present. Otherwise compute a stable fingerprint from account, date, direction, amount, normalized description, and relevant reference. Treat fingerprints as likely-duplicate signals, not infallible proof; provide review for collisions. Enforce a database uniqueness strategy that permits legitimate repeated identical purchases when no trustworthy unique reference exists (e.g., batch plus row occurrence or a reviewed fingerprint table rather than a universal hard unique transaction key).
10. **Manual edits.** Preserve imported raw values. Store user overrides separately or in explicit canonical override fields with audit timestamps. A user correction must not be overwritten by a future re-import.
11. **Rules.** Merchant rules are deterministic, ordered by explicit priority, and have a stable explanation (e.g. matched description contains `SWIGGY`). User edits can offer to create a rule but must not silently create one.
12. **Uncertainty.** Unmatched category/merchant stays `Needs review`; do not fabricate certainty. Initial rule suggestions may be based on narration text and user-approved rules only.
13. **Period comparison.** Compare like-for-like calendar periods and disclose missing account/date coverage where known. Month boundaries follow the user's local calendar setting.
14. **Rounding.** Keep sums exact in paise; round only at display boundaries using standard currency formatting.

## 8. SQLite schema

Use versioned SQL migrations, foreign keys enabled on every connection, constraints, and indexes. Use ISO-8601 date/time strings in UTC for timestamps, and ISO calendar dates for transaction dates. IDs are opaque text IDs (UUID recommended) unless the implementation has a well-justified alternative.

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE accounts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  institution TEXT NOT NULL DEFAULT 'Canara Bank',
  account_label TEXT,                 -- user-chosen nickname; never store credentials
  currency TEXT NOT NULL DEFAULT 'INR' CHECK (currency = 'INR'),
  is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE categories (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('expense', 'income', 'transfer', 'uncategorized')),
  parent_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
  is_system INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
  is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0, 1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  UNIQUE(parent_id, name)
);

CREATE TABLE merchants (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL UNIQUE,
  default_category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE import_batches (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  source_type TEXT NOT NULL CHECK (source_type IN ('canara_csv', 'canara_xlsx', 'upi_export', 'manual')),
  original_filename TEXT NOT NULL,
  file_sha256 TEXT,                    -- integrity/deduplication aid; never a path or file contents
  imported_at TEXT NOT NULL,
  period_start TEXT,
  period_end TEXT,
  rows_seen INTEGER NOT NULL DEFAULT 0,
  rows_imported INTEGER NOT NULL DEFAULT 0,
  rows_rejected INTEGER NOT NULL DEFAULT 0,
  rows_skipped_duplicate INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL CHECK (status IN ('previewed', 'committed', 'failed', 'cancelled')),
  notes TEXT
);

CREATE TABLE transactions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  import_batch_id TEXT REFERENCES import_batches(id) ON DELETE SET NULL,
  transaction_date TEXT NOT NULL,       -- YYYY-MM-DD
  value_date TEXT,
  direction TEXT NOT NULL CHECK (direction IN ('debit', 'credit')),
  amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
  balance_after_paise INTEGER,
  raw_description TEXT NOT NULL,
  normalized_description TEXT NOT NULL DEFAULT '',
  merchant_id TEXT REFERENCES merchants(id) ON DELETE SET NULL,
  merchant_override TEXT,
  reference_raw TEXT,
  transaction_kind TEXT NOT NULL DEFAULT 'unknown'
    CHECK (transaction_kind IN ('expense', 'income', 'transfer', 'refund', 'reversal', 'fee', 'unknown')),
  category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
  classification_source TEXT NOT NULL DEFAULT 'unclassified'
    CHECK (classification_source IN ('user', 'rule', 'import', 'unclassified')),
  is_reviewed INTEGER NOT NULL DEFAULT 0 CHECK (is_reviewed IN (0, 1)),
  matched_transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  source_row_number INTEGER,
  fingerprint TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE merchant_rules (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  match_type TEXT NOT NULL CHECK (match_type IN ('contains', 'equals', 'starts_with')),
  pattern TEXT NOT NULL,
  merchant_id TEXT REFERENCES merchants(id) ON DELETE SET NULL,
  category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
  transaction_kind TEXT CHECK (transaction_kind IN ('expense', 'income', 'transfer', 'refund', 'reversal', 'fee')),
  priority INTEGER NOT NULL DEFAULT 100,
  is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE budgets (
  id TEXT PRIMARY KEY,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  month TEXT NOT NULL,                 -- YYYY-MM in local calendar
  amount_paise INTEGER NOT NULL CHECK (amount_paise >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(category_id, month)
);

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE INDEX idx_transactions_account_date ON transactions(account_id, transaction_date DESC);
CREATE INDEX idx_transactions_category_date ON transactions(category_id, transaction_date DESC);
CREATE INDEX idx_transactions_merchant_date ON transactions(merchant_id, transaction_date DESC);
CREATE INDEX idx_transactions_review_date ON transactions(is_reviewed, transaction_date DESC);
CREATE INDEX idx_transactions_batch ON transactions(import_batch_id);
CREATE INDEX idx_transactions_reference ON transactions(account_id, reference_raw);
CREATE INDEX idx_rules_enabled_priority ON merchant_rules(is_enabled, priority);
CREATE INDEX idx_budgets_month ON budgets(month);
```

Schema notes:

- Store exact raw narration and source row number so users can investigate a classification or import issue.
- Avoid storing full statement files by default. `original_filename` is descriptive metadata only; do not retain an absolute path that leaks unrelated personal filesystem structure.
- A user can delete an import batch and its rows only through an explicit, clearly explained action with confirmation. Preserve database integrity and offer backup first.
- Add migrations as small, tested SQL files. Never silently reset or replace an existing user's database when a migration fails.
- WAL mode is an implementation option, not a security feature. Enable only if backup/restore and close/flush behavior are correct.

## 9. Security and privacy

- Request only the native file-picker access needed for import and the app data directory needed for SQLite/backups. Keep Tauri capabilities narrowly scoped.
- No internet permissions or network dependency should be present in the production app unless a future, explicitly approved feature requires it. Verify the built app works with Wi-Fi disabled.
- Never collect or log bank passwords, UPI PINs, OTPs, full account numbers, statement contents, or raw transaction narrations. Errors should identify file, row, and reason without dumping sensitive row contents.
- Store database under macOS Application Support with restrictive file permissions where practical. Recommend macOS FileVault and a secured user account as the first-release protection for a local database.
- Be honest that plain SQLite is not application-level encrypted. Do not claim database encryption unless a tested encryption implementation is included. SQLCipher/key-management can be evaluated later; avoid hand-rolled encryption.
- Do not delete or alter the user's input file. Do not keep temporary statement copies after parsing. Ensure import buffers are released when finished.
- Backups and exports are sensitive. Keep them user-selected, local, clearly labelled, and never upload them. Provide a restore confirmation and validation before replacing current data.
- Do not include personal transaction data in screenshots, fixtures, logs, test snapshots, or issue reports. Tests use synthetic fixtures.
- Dependencies must be reviewed for license, maintenance, security posture, and any runtime network behavior. Keep lockfiles checked in.
- No telemetry or auto-update service in the initial release. Updating is manual and offline-friendly.

## 10. UI and interaction requirements

### Visual direction

- Dark-first interface; near-black/charcoal surfaces with one restrained accent (for example muted emerald) and clear semantic colors.
- Consistent spacing and typography; use native-feeling macOS window behavior and keyboard conventions.
- Stable layout with a compact left navigation rail/sidebar: Overview, Transactions, Budgets (when implemented), Settings.
- Use real empty states and sample data only in an explicitly labelled demo mode or tests. Never seed fake transactions into the actual account.
- Numbers are legible at a glance; category charts include labels/legend and accessible text summaries. Do not rely on color alone.
- Animation is optional, short, and respects reduced-motion settings.

### Required screens

1. **Overview:** selected period, total expenses, income, net cash movement, category breakdown, trend/comparison, unreviewed count, data coverage/last import.
2. **Transactions:** paginated or virtualised list, search and filters, amount/direction, date, merchant, category, kind, review marker, source/batch details, edit/review action.
3. **Import:** file selection, account, sheet/column mapping, preview, validation feedback, duplicate review, explicit commit, result summary.
4. **Budgets:** P1; category budget, spent, remaining, progress, remaining/day with the calculation explained.
5. **Settings:** accounts, categories, rules, currency/month preference, backup/restore/export, privacy and local database location.

## 11. Performance and reliability budgets

Targets are measured on the developer's Mac and reported with hardware/macOS context. They are budgets for investigation, not claims that Tauri automatically guarantees performance.

- Cold launch to usable first screen: **≤ 2.5 seconds** on the target Mac, median over 5 launches after the first run; show a useful loading state if database migration or first launch takes longer.
- Idle CPU after load: **near 0% sustained**; no polling or recurring background task.
- Memory after 60 seconds idle with no statement loaded: target **≤ 250 MB aggregate app process memory** on the target Mac. Record actual results; if exceeded, inspect assets, charts, duplicate WebViews and dependencies before relaxing the target.
- With 50,000 synthetic transactions: filtered first-page transaction query **≤ 300 ms**; dashboard summary **≤ 500 ms** after database warm-up; import preview should remain responsive and avoid blocking UI while parsing.
- 50,000-row CSV import: complete parse/preview/commit in **≤ 10 seconds** on the target Mac, excluding interactive review. XLSX performance may differ and should be separately recorded.
- No unnecessary animation, interval polling, unbounded in-memory transaction array, or repeated full-history aggregation on every render.
- Use pagination or virtualization for transaction tables and indexed SQL filters. Aggregate in SQL where it improves measured performance.
- App remains usable with no network. Recover from malformed files, cancellation, disk full, and database errors with actionable messages and no partial import.

## 12. Testing and verification

Automated tests are required by the implementation brief. Run them incrementally; do not defer all verification to the end.

### Backend tests

- Amount/date/encoding/column mapping tests for CSV and XLSX fixtures.
- Invalid rows and error reporting with row numbers.
- Debit/credit and transaction-kind accounting tests.
- Transfer exclusion, fee inclusion, refund/reversal handling, and linked-refund tests.
- Fingerprint and duplicate-candidate tests, including identical legitimate purchases.
- Merchant-rule priority, explainability, and user override persistence tests.
- Migration from empty and prior-version databases; rollback on migration/import failure.
- Atomic batch import and backup/restore tests.
- SQL query and aggregate tests against synthetic datasets.

### Frontend tests

- Empty state, import preview and confirmation flow.
- Invalid-row and duplicate summaries.
- Transaction filtering/search and manual review/edit.
- Currency formatting and accessible chart summaries.
- Error states for cancelled import, unavailable database, and restore confirmation.

### Build and manual checks

- `cargo fmt --check`, `cargo clippy` (or documented equivalent), Rust tests.
- Frontend lint, typecheck, and unit/component tests.
- Tauri development build and production macOS bundle build at the specified milestones.
- Open the produced `.app` from Finder, quit/relaunch it, and confirm no terminal/browser/server is needed.
- Disconnect network and verify core workflows still work.
- Test on synthetic data including an empty account, 50,000 transactions, malformed files, repeated imports, and backup/restore.
- Inspect app capabilities and built output for unexpected network access, telemetry, or bundled services.
- Check keyboard navigation, readable contrast, and reduced-motion behavior.

Do not use the user's real bank statement for automated tests. A redacted sample may be used manually only with the user's decision; synthetic fixtures are the default.

## 13. Project structure

```text
ledger/
├── README.md
├── PROJECT_SPEC.md
├── EXECUTION_PROMPT.md
├── package.json
├── package-lock.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   ├── app/
│   │   ├── App.tsx
│   │   ├── routes.ts
│   │   └── commands.ts          # typed wrappers for Tauri commands
│   ├── components/
│   │   ├── layout/
│   │   ├── charts/
│   │   ├── transactions/
│   │   └── ui/
│   ├── features/
│   │   ├── overview/
│   │   ├── transactions/
│   │   ├── imports/
│   │   ├── budgets/
│   │   └── settings/
│   ├── styles/
│   │   ├── tokens.css
│   │   └── global.css
│   └── test/
│       └── fixtures/            # synthetic only
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   ├── icons/
│   ├── migrations/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       ├── domain/
│       │   ├── transaction.rs
│       │   ├── accounting.rs
│       │   ├── categories.rs
│       │   └── rules.rs
│       ├── imports/
│       │   ├── mod.rs
│       │   ├── csv.rs
│       │   ├── xlsx.rs
│       │   ├── mapping.rs
│       │   └── normalize.rs
│       ├── db/
│       │   ├── mod.rs
│       │   ├── migrations.rs
│       │   └── repositories.rs
│       └── services/
├── docs/
│   ├── decisions.md
│   └── sample-import-format.md
└── .gitignore
```

This is a target structure, not a mandate to create empty directories. Create modules as each milestone needs them. Keep command interfaces narrow and domain logic testable without launching the UI.

## 14. Phased implementation plan and gates

Every milestone ends with: a short summary of changes, exact verification commands and results, known limitations, and the next milestone proposal. Do not proceed past a failed gate. Fix the issue or report a blocker with evidence.

### Milestone 0 — Verify toolchain and establish project

- Check macOS version/architecture and available Rust, Node, Xcode command-line tools, and Tauri prerequisites.
- If a prerequisite is absent, report what is missing and the official installation step. Do not install system-wide tools or change machine configuration without direction.
- Scaffold a minimal Tauri 2 + React + TypeScript app. Commit/retain lockfiles. Confirm an app window opens and a production `.app` can be created.
- Add app identity, restrictive capabilities, dark theme tokens, README with build steps.

**Gate:** frontend typecheck/lint, Rust format/test/build, Tauri dev launch, macOS bundle build; launch bundled app from Finder. Confirm no localhost service is required at runtime.

### Milestone 1 — Local database and domain foundation

- Add SQLite open/init, migrations, default categories, account setup, repository boundary, and synthetic test database.
- Implement canonical transaction types and money/date parsing helpers.

**Gate:** migration, constraints, exact paise, and repository tests pass; app still packages and relaunches.

### Milestone 2 — CSV import with preview and atomic commit

- CSV selection, mapping proposal/configuration, validation, preview, duplicate candidates, explicit confirmation, atomic import, batch history.
- Use synthetic Canara-like fixtures, then confirm header presets only from a redacted real-format example if available.

**Gate:** import/accounting/duplicate tests pass; repeated import does not silently double-count; cancellation and malformed-file paths work; bundle opens.

### Milestone 3 — XLSX import and transaction manager

- Worksheet selection, XLSX mapping/preview, transaction table with search/filter/pagination, manual category/merchant editing, review state.

**Gate:** XLSX and UI tests pass; 50,000-row table stays responsive; all listed filters work; bundle opens.

### Milestone 4 — Minimal polished dashboard

- Implement real-data overview, category chart, period comparison, coverage/last-import indicators, useful empty state.
- Keep summaries in Rust/SQLite query layer and avoid fabricated demo records.

**Gate:** aggregate tests and UI checks pass; manually reconcile dashboard figures to known synthetic entries; verify performance budgets.

### Milestone 5 — Rules and budgets

- Deterministic merchant rules with explanations, category budgets, progress and remaining/day calculations.

**Gate:** rule precedence and budget edge-case tests pass; user overrides survive future imports.

### Milestone 6 — Privacy, backup, recovery, release

- CSV export, validated backup/restore, data deletion controls, privacy documentation, production branding, `.app` and `.dmg` packaging.
- Inspect entitlements/capabilities and ensure there are no network or telemetry dependencies.
- Test clean install/open, quit/relaunch, backup/restore, offline operation, and migration behavior.

**Gate:** full test suite and macOS bundle build pass; package and user guide are present; report signing/notarisation status accurately.

### Optional Milestone 7 — Google Pay/BHIM enrichment

Only begin after the user has a real, redacted export or a stable publicly documented export format. Implement as isolated parsers and staged matching; do not delay the bank-statement-based app. Never add login automation or scraping.

## 15. Release and distribution plan

- First target the Mac's current CPU architecture, discovered at Milestone 0. Avoid universal builds until the basic release is stable.
- Use Tauri's macOS bundle tooling to produce a self-contained `.app`; create a `.dmg` containing the app and a concise installation note.
- The app must be launchable from Applications or Finder without terminal commands, browser tabs, localhost, Python, or Node at runtime.
- Keep installer artifacts and backups outside the repository unless explicitly requested; never include personal data.
- For personal use, an unsigned/ad-hoc-signed local build may be usable but macOS may warn or require the user to approve opening it. Document exact behavior. Do not claim Gatekeeper-ready public distribution without Developer ID signing and notarisation.
- Signing/notarisation availability, developer account requirements, and costs must be checked at release time. Keep them outside the ₹0 recurring software/service target and describe any optional Apple membership expense clearly.
- Version database schema independently from app version. Back up before destructive migration or restore. Never silently erase a user's database during upgrades.
- Supply `README.md` with installation, first launch, backup, restore, privacy, known limitations, and uninstall/data-retention steps.

## 16. Acceptance criteria for the initial usable release

The release is accepted only when all applicable criteria are demonstrated:

### Product and runtime

- [ ] Opens as a packaged macOS `.app` from Finder; `.dmg` is available.
- [ ] No localhost server, browser, terminal, Python, account, network, or paid API is needed for core use.
- [ ] Core features work offline and no financial data leaves the Mac.
- [ ] UI is dark, minimal, polished, and has real empty/error/loading states without seeded fake financial transactions.

### Data correctness

- [ ] Canara-like CSV and XLSX can be mapped, previewed, validated, and imported.
- [ ] Import is explicit and atomic; malformed rows are reported with row numbers.
- [ ] Money is stored in paise and displayed as INR.
- [ ] Reimport reports likely duplicates and does not silently double-count trusted duplicate references.
- [ ] Transfers are excluded from expense/income; refund/reversal are not income; fees and expense totals follow the documented rules.
- [ ] Unknown classifications remain visible for review; user edits persist.
- [ ] Dashboard totals reconcile against synthetic fixtures and expose incomplete coverage where known.

### Privacy, safety, reliability

- [ ] No telemetry, analytics, remote assets, network dependency, credentials collection, or sensitive-data logging.
- [ ] Database location and backup/export behavior are clear; restore is validated and confirmed.
- [ ] Migrations and imports roll back safely on failure; existing data is not silently reset.
- [ ] Automated tests and static checks pass; production bundle is manually opened and relaunched.
- [ ] Performance results are recorded against the targets in Section 11 with hardware/macOS context.
