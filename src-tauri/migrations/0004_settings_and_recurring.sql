CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE dismissed_recurring (
  merchant_key TEXT PRIMARY KEY,
  dismissed_at TEXT NOT NULL
);
