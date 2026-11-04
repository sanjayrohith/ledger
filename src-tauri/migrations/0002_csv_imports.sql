CREATE TABLE import_batches (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  source_type TEXT NOT NULL CHECK (source_type IN ('canara_csv', 'canara_xlsx', 'upi_export', 'manual')),
  original_filename TEXT NOT NULL,
  file_sha256 TEXT,
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
  transaction_date TEXT NOT NULL,
  value_date TEXT,
  direction TEXT NOT NULL CHECK (direction IN ('debit', 'credit')),
  amount_paise INTEGER NOT NULL CHECK (amount_paise > 0),
  balance_after_paise INTEGER,
  raw_description TEXT NOT NULL,
  normalized_description TEXT NOT NULL DEFAULT '',
  merchant_id TEXT,
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

CREATE INDEX idx_transactions_account_date ON transactions(account_id, transaction_date DESC);
CREATE INDEX idx_transactions_batch ON transactions(import_batch_id);
CREATE INDEX idx_transactions_reference ON transactions(account_id, reference_raw);
