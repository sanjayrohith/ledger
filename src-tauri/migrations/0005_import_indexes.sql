CREATE INDEX idx_transactions_fingerprint ON transactions(account_id, fingerprint);
CREATE INDEX idx_transactions_date ON transactions(transaction_date);
