use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use time::{format_description::well_known::Iso8601, Date};

use crate::db::{Database, DbError};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchSummary {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub filename: String,
    pub imported_at: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub rows_seen: i64,
    pub rows_imported: i64,
    pub rows_skipped_duplicate: i64,
    pub rows_rejected: i64,
    /// Transactions from this import that are still in Ledger.
    pub transactions_remaining: i64,
    /// Of those, how many you reviewed or categorised yourself.
    pub edited_by_you: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RangeSummary {
    pub count: i64,
    pub edited_by_you: i64,
    pub debit_total_paise: i64,
    pub credit_total_paise: i64,
}

pub struct ImportHistoryRepository<'database> {
    database: &'database Database,
}

const EDITED_SQL: &str = "(classification_source = 'user' OR is_reviewed = 1)";

impl<'database> ImportHistoryRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn list(&self) -> Result<Vec<ImportBatchSummary>, DbError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT b.id, b.account_id, a.name, b.original_filename, b.imported_at,
                        b.period_start, b.period_end, b.rows_seen, b.rows_imported,
                        b.rows_skipped_duplicate, b.rows_rejected,
                        (SELECT COUNT(*) FROM transactions t WHERE t.import_batch_id = b.id),
                        (SELECT COUNT(*) FROM transactions t WHERE t.import_batch_id = b.id AND {EDITED_SQL})
                 FROM import_batches b JOIN accounts a ON a.id = b.account_id
                 WHERE b.status = 'committed'
                 ORDER BY b.imported_at DESC, b.id"
            ))?;
            let rows = statement
                .query_map([], |row| {
                    Ok(ImportBatchSummary {
                        id: row.get(0)?,
                        account_id: row.get(1)?,
                        account_name: row.get(2)?,
                        filename: row.get(3)?,
                        imported_at: row.get(4)?,
                        period_start: row.get(5)?,
                        period_end: row.get(6)?,
                        rows_seen: row.get(7)?,
                        rows_imported: row.get(8)?,
                        rows_skipped_duplicate: row.get(9)?,
                        rows_rejected: row.get(10)?,
                        transactions_remaining: row.get(11)?,
                        edited_by_you: row.get(12)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Deletes one import and the transactions it added. Returns how many transactions went.
    pub fn delete_batch(&self, id: &str) -> Result<usize, DbError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let exists: Option<i64> = transaction
                .query_row("SELECT 1 FROM import_batches WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?;
            if exists.is_none() {
                return Err(DbError::NotFound);
            }
            let deleted =
                transaction.execute("DELETE FROM transactions WHERE import_batch_id = ?1", [id])?;
            transaction.execute("DELETE FROM import_batches WHERE id = ?1", [id])?;
            transaction.commit()?;
            Ok(deleted)
        })
    }

    pub fn range_summary(
        &self,
        account_id: Option<&str>,
        from: &str,
        to: &str,
    ) -> Result<RangeSummary, DbError> {
        validate_range(from, to)?;
        let account = account_id.filter(|value| !value.is_empty());
        self.database
            .with_connection(|connection| range_summary(connection, account, from, to))
    }

    /// Deletes every transaction dated within `from..=to` (optionally for one account).
    pub fn delete_range(
        &self,
        account_id: Option<&str>,
        from: &str,
        to: &str,
    ) -> Result<usize, DbError> {
        validate_range(from, to)?;
        let account = account_id.filter(|value| !value.is_empty());
        self.database.with_connection(|connection| {
            let deleted = connection.execute(
                "DELETE FROM transactions
                 WHERE transaction_date >= ?1 AND transaction_date <= ?2
                   AND (?3 IS NULL OR account_id = ?3)",
                params![from, to, account],
            )?;
            Ok(deleted)
        })
    }
}

fn range_summary(
    connection: &Connection,
    account: Option<&str>,
    from: &str,
    to: &str,
) -> Result<RangeSummary, DbError> {
    Ok(connection.query_row(
        &format!(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN {EDITED_SQL} THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN direction = 'debit' THEN amount_paise ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN direction = 'credit' THEN amount_paise ELSE 0 END), 0)
             FROM transactions
             WHERE transaction_date >= ?1 AND transaction_date <= ?2
               AND (?3 IS NULL OR account_id = ?3)"
        ),
        params![from, to, account],
        |row| {
            Ok(RangeSummary {
                count: row.get(0)?,
                edited_by_you: row.get(1)?,
                debit_total_paise: row.get(2)?,
                credit_total_paise: row.get(3)?,
            })
        },
    )?)
}

fn validate_range(from: &str, to: &str) -> Result<(), DbError> {
    let parse = |value: &str, label: &str| {
        Date::parse(value, &Iso8601::DATE)
            .map_err(|_| DbError::Validation(format!("{label} date must look like 2026-03-31")))
    };
    let (start, end) = (parse(from, "start")?, parse(to, "end")?);
    if start > end {
        return Err(DbError::Validation(
            "start date must not be after the end date".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::ImportHistoryRepository;
    use crate::db::{
        repositories::{AccountDraft, AccountRepository, TransactionEdit, TransactionRepository},
        Database, DbError,
    };
    use crate::imports::csv::CsvImportService;

    fn file(name: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ledger-{name}-{}.csv", uuid::Uuid::new_v4()));
        fs::write(&path, body).unwrap();
        path
    }

    fn import(database: &Database, account: &str, path: &Path) -> (usize, usize, usize) {
        let service = CsvImportService::new(database);
        let text = path.to_str().unwrap();
        let preview = service.preview(account, text, None).unwrap();
        let result = service.commit(account, text, preview.mapping).unwrap();
        (
            preview.rows_new,
            result.rows_imported,
            result.rows_skipped_duplicate,
        )
    }

    fn count(database: &Database) -> i64 {
        database
            .with_connection(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
            })
            .unwrap()
    }

    const HEADER: &str = "Transaction Date,Narration,Withdrawal Amt.,Deposit Amt.,UTR\n";

    #[test]
    fn weekly_overlapping_files_add_only_new_rows_with_or_without_references() {
        let database = Database::open_in_memory().unwrap();
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Daily".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        // Week 1: two identical coffees on one day (no UTR), plus a referenced payment.
        let week1 = file("w1", &format!(
            "{HEADER}2026-03-02,COFFEE SHOP,120.00,,\n2026-03-02,COFFEE SHOP,120.00,,\n2026-03-03,RENT,9000.00,,UTR-1\n"
        ));
        assert_eq!(import(&database, &account.id, &week1), (3, 3, 0));

        // Week 2 repeats week 1 and adds a third identical coffee, a new payment and a new referenced one.
        let week2 = file("w2", &format!(
            "{HEADER}2026-03-02,COFFEE SHOP,120.00,,\n2026-03-02,COFFEE SHOP,120.00,,\n2026-03-03,RENT,9000.00,,UTR-1\n\
             2026-03-09,COFFEE SHOP,120.00,,\n2026-03-09,BOOKS,450.00,,\n2026-03-10,POWER BILL,1300.00,,UTR-2\n"
        ));
        let service = CsvImportService::new(&database);
        let preview = service
            .preview(&account.id, week2.to_str().unwrap(), None)
            .unwrap();
        assert_eq!((preview.rows_new, preview.rows_already_imported), (3, 3));
        assert_eq!(preview.new_debit_total_paise, 12_000 + 45_000 + 130_000);
        assert_eq!(import(&database, &account.id, &week2), (3, 3, 3));
        assert_eq!(count(&database), 6);

        // Uploading the same week 2 file again adds nothing.
        assert_eq!(import(&database, &account.id, &week2), (0, 0, 6));
        assert_eq!(count(&database), 6);
    }

    #[test]
    fn same_file_in_another_account_is_not_treated_as_already_imported() {
        let database = Database::open_in_memory().unwrap();
        let accounts = AccountRepository::new(&database);
        let one = accounts
            .create(AccountDraft {
                name: "One".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let two = accounts
            .create(AccountDraft {
                name: "Two".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let path = file("acct", &format!("{HEADER}2026-03-02,SHOP,10.00,,\n"));
        assert_eq!(import(&database, &one.id, &path).1, 1);
        assert_eq!(import(&database, &two.id, &path).1, 1);
    }

    #[test]
    fn delete_import_removes_its_rows_and_allows_a_clean_reupload() {
        let database = Database::open_in_memory().unwrap();
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Daily".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let path = file(
            "del",
            &format!("{HEADER}2026-03-02,SHOP,10.00,,\n2026-03-03,CAFE,20.00,,U1\n"),
        );
        import(&database, &account.id, &path);
        let history = ImportHistoryRepository::new(&database);
        let batches = history.list().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(
            (batches[0].transactions_remaining, batches[0].edited_by_you),
            (2, 0)
        );
        assert_eq!(batches[0].account_name, "Daily");

        let transactions = TransactionRepository::new(&database)
            .page(crate::db::repositories::TransactionQuery {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        TransactionRepository::new(&database)
            .edit(
                &transactions.items[0].id,
                TransactionEdit {
                    merchant_override: Some("Mine".into()),
                    category_id: None,
                    is_reviewed: true,
                },
            )
            .unwrap();
        assert_eq!(history.list().unwrap()[0].edited_by_you, 1);

        assert_eq!(history.delete_batch(&batches[0].id).unwrap(), 2);
        assert_eq!(count(&database), 0);
        assert!(history.list().unwrap().is_empty());
        assert!(matches!(
            history.delete_batch(&batches[0].id),
            Err(DbError::NotFound)
        ));
        assert_eq!(import(&database, &account.id, &path), (2, 2, 0));
    }

    #[test]
    fn delete_by_date_range_is_inclusive_scoped_to_an_account_and_validated() {
        let database = Database::open_in_memory().unwrap();
        let accounts = AccountRepository::new(&database);
        let one = accounts
            .create(AccountDraft {
                name: "One".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let two = accounts
            .create(AccountDraft {
                name: "Two".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let path = file("range", &format!(
            "{HEADER}2026-03-01,A,10.00,,\n2026-03-07,B,20.00,,\n2026-03-08,C,,30.00,\n2026-03-20,D,40.00,,\n"
        ));
        import(&database, &one.id, &path);
        import(&database, &two.id, &path);
        let history = ImportHistoryRepository::new(&database);

        let summary = history
            .range_summary(Some(&one.id), "2026-03-01", "2026-03-07")
            .unwrap();
        assert_eq!(
            (
                summary.count,
                summary.debit_total_paise,
                summary.credit_total_paise
            ),
            (2, 3000, 0)
        );
        let both = history
            .range_summary(None, "2026-03-01", "2026-03-31")
            .unwrap();
        assert_eq!(both.count, 8);

        assert_eq!(
            history
                .delete_range(Some(&one.id), "2026-03-01", "2026-03-07")
                .unwrap(),
            2
        );
        assert_eq!(count(&database), 6);
        assert_eq!(
            history
                .delete_range(None, "2026-03-08", "2026-03-08")
                .unwrap(),
            2
        );
        assert_eq!(count(&database), 4);

        assert!(history
            .delete_range(None, "2026-03-09", "2026-03-01")
            .is_err());
        assert!(history.delete_range(None, "March", "2026-03-01").is_err());
        assert_eq!(count(&database), 4);

        // After deleting a range, re-uploading restores only the missing rows.
        assert_eq!(import(&database, &one.id, &path), (3, 3, 1));
    }
}
