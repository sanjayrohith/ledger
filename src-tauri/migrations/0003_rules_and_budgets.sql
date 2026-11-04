CREATE TABLE merchants (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL UNIQUE,
  default_category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
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
  month TEXT NOT NULL,
  amount_paise INTEGER NOT NULL CHECK (amount_paise >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(category_id, month)
);

CREATE INDEX idx_rules_enabled_priority ON merchant_rules(is_enabled, priority, created_at, id);
CREATE INDEX idx_budgets_month ON budgets(month);
