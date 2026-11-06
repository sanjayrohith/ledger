use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Database, DbError};
use crate::domain::transaction::utc_timestamp;

pub const FIRST_DAY_KEY: &str = "first_day_of_month";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub first_day_of_month: u8,
    pub currency: String,
    pub database_path: Option<String>,
}

pub struct SettingsRepository<'database> {
    database: &'database Database,
}

impl<'database> SettingsRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn first_day_of_month(&self) -> Result<u8, DbError> {
        self.database.with_connection(|connection| {
            let value: Option<String> = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    [FIRST_DAY_KEY],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(value
                .and_then(|text| text.parse::<u8>().ok())
                .filter(|day| (1..=28).contains(day))
                .unwrap_or(1))
        })
    }

    pub fn set_first_day_of_month(&self, day: u8) -> Result<(), DbError> {
        if !(1..=28).contains(&day) {
            return Err(DbError::Validation(
                "first day of month must be between 1 and 28".into(),
            ));
        }
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![FIRST_DAY_KEY, day.to_string()],
            )?;
            Ok(())
        })
    }

    pub fn load(&self) -> Result<AppSettings, DbError> {
        Ok(AppSettings {
            first_day_of_month: self.first_day_of_month()?,
            currency: "INR".into(),
            database_path: self
                .database
                .database_path()
                .map(|path| path.display().to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_system: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDraft {
    pub name: String,
    pub kind: String,
}

pub struct CategoryAdminRepository<'database> {
    database: &'database Database,
}

impl<'database> CategoryAdminRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn list_all(&self) -> Result<Vec<CategoryRow>, DbError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, name, kind, is_system FROM categories WHERE is_archived = 0
                 ORDER BY CASE kind WHEN 'expense' THEN 0 WHEN 'income' THEN 1 WHEN 'transfer' THEN 2 ELSE 3 END, sort_order, name",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok(CategoryRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        kind: row.get(2)?,
                        is_system: row.get::<_, i64>(3)? == 1,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn create(&self, draft: CategoryDraft) -> Result<CategoryRow, DbError> {
        let name = draft.name.trim().to_owned();
        if name.is_empty() {
            return Err(DbError::Validation("category name is required".into()));
        }
        if !["expense", "income"].contains(&draft.kind.as_str()) {
            return Err(DbError::Validation(
                "custom categories must be expense or income".into(),
            ));
        }
        self.database.with_connection(|connection| {
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM categories WHERE lower(name) = lower(?1) AND is_archived = 0)",
                [&name],
                |row| row.get(0),
            )?;
            if exists {
                return Err(DbError::Validation(format!("category '{name}' already exists")));
            }
            let id = Uuid::new_v4().to_string();
            connection.execute(
                "INSERT INTO categories (id, name, kind, is_system, sort_order) VALUES (?1, ?2, ?3, 0, 100)",
                params![id, name, draft.kind],
            )?;
            Ok(CategoryRow { id, name, kind: draft.kind, is_system: false })
        })
    }

    pub fn rename(&self, id: &str, name: String) -> Result<(), DbError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(DbError::Validation("category name is required".into()));
        }
        self.database.with_connection(|connection| {
            let is_system: Option<i64> = connection
                .query_row(
                    "SELECT is_system FROM categories WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            match is_system {
                None => Err(DbError::NotFound),
                Some(1) => Err(DbError::Validation(
                    "built-in categories cannot be renamed".into(),
                )),
                Some(_) => {
                    connection.execute(
                        "UPDATE categories SET name = ?1 WHERE id = ?2",
                        params![name, id],
                    )?;
                    Ok(())
                }
            }
        })
    }

    /// Archives a custom category. Transactions keep their historical category;
    /// budgets and rules that point at it are removed so nothing refers to a hidden category.
    pub fn archive(&self, id: &str) -> Result<(), DbError> {
        self.database.with_connection(|connection| {
            let is_system: Option<i64> = connection
                .query_row(
                    "SELECT is_system FROM categories WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            match is_system {
                None => Err(DbError::NotFound),
                Some(1) => Err(DbError::Validation(
                    "built-in categories cannot be archived".into(),
                )),
                Some(_) => {
                    connection
                        .execute("UPDATE categories SET is_archived = 1 WHERE id = ?1", [id])?;
                    connection.execute("DELETE FROM budgets WHERE category_id = ?1", [id])?;
                    connection.execute(
                        "UPDATE merchant_rules SET category_id = NULL WHERE category_id = ?1",
                        [id],
                    )?;
                    Ok(())
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSuggestion {
    pub merchant_key: String,
    pub merchant: String,
    pub typical_amount_paise: i64,
    pub months_seen: i64,
    pub last_seen: String,
}

pub struct RecurringRepository<'database> {
    database: &'database Database,
}

impl<'database> RecurringRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    /// Merchants charged in at least three different months with amounts within 15% of each
    /// other. These are suggestions only; nothing is created or changed unless the user acts.
    pub fn suggestions(&self) -> Result<Vec<RecurringSuggestion>, DbError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT lower(COALESCE(NULLIF(merchant_override,''), normalized_description)) AS key,
                        COALESCE(NULLIF(merchant_override,''), raw_description),
                        COUNT(DISTINCT substr(transaction_date,1,7)),
                        MIN(amount_paise), MAX(amount_paise), CAST(AVG(amount_paise) AS INTEGER),
                        MAX(transaction_date)
                 FROM transactions
                 WHERE direction='debit' AND transaction_kind IN ('expense','fee')
                 GROUP BY key
                 HAVING COUNT(DISTINCT substr(transaction_date,1,7)) >= 3
                    AND MAX(amount_paise) * 100 <= MIN(amount_paise) * 115
                    AND key NOT IN (SELECT merchant_key FROM dismissed_recurring)
                 ORDER BY 6 DESC
                 LIMIT 20",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok(RecurringSuggestion {
                        merchant_key: row.get(0)?,
                        merchant: row.get(1)?,
                        months_seen: row.get(2)?,
                        typical_amount_paise: row.get(5)?,
                        last_seen: row.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn dismiss(&self, merchant_key: &str) -> Result<(), DbError> {
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO dismissed_recurring (merchant_key, dismissed_at) VALUES (?1, ?2)",
                params![merchant_key, utc_timestamp()],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(database: &Database, id: &str, date: &str, amount: i64, merchant: &str) {
        database.with_connection(|connection| {
            connection.execute("INSERT OR IGNORE INTO accounts (id,name,institution,created_at,updated_at) VALUES ('a','Synthetic','Canara Bank','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", [])?;
            connection.execute("INSERT INTO transactions (id,account_id,transaction_date,direction,amount_paise,raw_description,normalized_description,transaction_kind,is_reviewed,fingerprint,created_at,updated_at) VALUES (?1,'a',?2,'debit',?3,?4,lower(?4),'expense',0,?1,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", params![id, date, amount, merchant])?;
            Ok(())
        }).unwrap();
    }

    #[test]
    fn first_day_defaults_to_one_and_validates_range() {
        let database = Database::open_in_memory().unwrap();
        let settings = SettingsRepository::new(&database);
        assert_eq!(settings.first_day_of_month().unwrap(), 1);
        settings.set_first_day_of_month(5).unwrap();
        assert_eq!(settings.first_day_of_month().unwrap(), 5);
        assert!(settings.set_first_day_of_month(0).is_err());
        assert!(settings.set_first_day_of_month(29).is_err());
        assert_eq!(settings.first_day_of_month().unwrap(), 5);
    }

    #[test]
    fn custom_categories_can_be_created_renamed_and_archived_but_builtin_ones_are_protected() {
        let database = Database::open_in_memory().unwrap();
        let repo = CategoryAdminRepository::new(&database);
        let builtin = repo
            .list_all()
            .unwrap()
            .into_iter()
            .find(|c| c.is_system)
            .unwrap();
        assert!(repo.rename(&builtin.id, "Nope".into()).is_err());
        assert!(repo.archive(&builtin.id).is_err());
        let custom = repo
            .create(CategoryDraft {
                name: " Pets ".into(),
                kind: "expense".into(),
            })
            .unwrap();
        assert_eq!(custom.name, "Pets");
        assert!(repo
            .create(CategoryDraft {
                name: "pets".into(),
                kind: "expense".into()
            })
            .is_err());
        assert!(repo
            .create(CategoryDraft {
                name: "X".into(),
                kind: "transfer".into()
            })
            .is_err());
        repo.rename(&custom.id, "Pet care".into()).unwrap();
        assert!(repo
            .list_all()
            .unwrap()
            .iter()
            .any(|c| c.name == "Pet care"));
        repo.archive(&custom.id).unwrap();
        assert!(!repo.list_all().unwrap().iter().any(|c| c.id == custom.id));
    }

    #[test]
    fn recurring_needs_three_months_similar_amounts_and_respects_dismissal() {
        let database = Database::open_in_memory().unwrap();
        for (i, date) in ["2026-01-05", "2026-02-05", "2026-03-06"]
            .iter()
            .enumerate()
        {
            insert(
                &database,
                &format!("n{i}"),
                date,
                64900 + i as i64 * 100,
                "Netflix",
            );
        }
        for (i, date) in ["2026-01-09", "2026-02-09", "2026-03-09"]
            .iter()
            .enumerate()
        {
            insert(
                &database,
                &format!("v{i}"),
                date,
                10000 * (i as i64 + 1),
                "Varying",
            );
        }
        insert(&database, "o1", "2026-03-01", 500, "Once");
        let repo = RecurringRepository::new(&database);
        let found = repo.suggestions().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].merchant, "Netflix");
        assert_eq!(found[0].months_seen, 3);
        repo.dismiss(&found[0].merchant_key).unwrap();
        assert!(repo.suggestions().unwrap().is_empty());
    }
}
