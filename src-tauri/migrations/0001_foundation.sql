CREATE TABLE accounts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  institution TEXT NOT NULL DEFAULT 'Canara Bank',
  account_label TEXT,
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

