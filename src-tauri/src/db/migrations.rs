use rusqlite::{params, Connection, Transaction};

use super::DbError;
use crate::domain::transaction::utc_timestamp;

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../migrations/0001_foundation.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../migrations/0002_csv_imports.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../migrations/0003_rules_and_budgets.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../migrations/0004_settings_and_recurring.sql"),
    },
];

const DEFAULT_CATEGORIES: &[(&str, &str, &str, i64)] = &[
    (
        "00000000-0000-4000-8000-000000000001",
        "Groceries",
        "expense",
        10,
    ),
    (
        "00000000-0000-4000-8000-000000000002",
        "Dining",
        "expense",
        20,
    ),
    (
        "00000000-0000-4000-8000-000000000003",
        "Transport",
        "expense",
        30,
    ),
    (
        "00000000-0000-4000-8000-000000000004",
        "Shopping",
        "expense",
        40,
    ),
    (
        "00000000-0000-4000-8000-000000000005",
        "Utilities",
        "expense",
        50,
    ),
    (
        "00000000-0000-4000-8000-000000000006",
        "Health",
        "expense",
        60,
    ),
    (
        "00000000-0000-4000-8000-000000000007",
        "Entertainment",
        "expense",
        70,
    ),
    (
        "00000000-0000-4000-8000-000000000008",
        "Other",
        "expense",
        80,
    ),
    (
        "00000000-0000-4000-8000-000000000009",
        "Salary",
        "income",
        10,
    ),
    (
        "00000000-0000-4000-8000-000000000010",
        "Other income",
        "income",
        20,
    ),
    (
        "00000000-0000-4000-8000-000000000011",
        "Own-account transfer",
        "transfer",
        10,
    ),
    (
        "00000000-0000-4000-8000-000000000012",
        "Uncategorised",
        "uncategorized",
        10,
    ),
];

pub fn apply(connection: &mut Connection) -> Result<(), DbError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    let transaction = connection.transaction()?;
    for migration in MIGRATIONS {
        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;
        if !already_applied {
            transaction.execute_batch(migration.sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![migration.version, utc_timestamp()],
            )?;
        }
    }

    seed_default_categories(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn seed_default_categories(transaction: &Transaction<'_>) -> Result<(), DbError> {
    for (id, name, kind, sort_order) in DEFAULT_CATEGORIES {
        transaction.execute(
            "INSERT OR IGNORE INTO categories (id, name, kind, is_system, sort_order)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![id, name, kind, sort_order],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::apply;

    #[test]
    fn migration_creates_versioned_schema_and_seeds_categories_once() {
        let mut connection = Connection::open_in_memory().expect("synthetic database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys enabled");

        apply(&mut connection).expect("first migration succeeds");
        apply(&mut connection).expect("repeat migration succeeds");

        let version_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration rows query");
        let category_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .expect("category rows query");

        assert_eq!(version_count, 4);
        assert_eq!(category_count, 12);
    }

    #[test]
    fn constraints_reject_invalid_currency_and_category_parent() {
        let mut connection = Connection::open_in_memory().expect("synthetic database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys enabled");
        apply(&mut connection).expect("migration succeeds");

        let invalid_currency = connection.execute(
            "INSERT INTO accounts (id, name, currency, created_at, updated_at)
             VALUES ('synthetic-account', 'Synthetic', 'USD', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        );
        let invalid_parent = connection.execute(
            "INSERT INTO categories (id, name, kind, parent_id)
             VALUES ('synthetic-category', 'Synthetic', 'expense', 'missing-parent')",
            [],
        );

        assert!(invalid_currency.is_err());
        assert!(invalid_parent.is_err());
    }
}
