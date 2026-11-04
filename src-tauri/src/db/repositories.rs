use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Database, DbError};
use crate::domain::transaction::utc_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub institution: String,
    pub account_label: Option<String>,
    pub currency: String,
    pub is_archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionListItem {
    pub id: String,
    pub transaction_date: String,
    pub direction: String,
    pub amount_paise: i64,
    pub raw_description: String,
    pub merchant_override: Option<String>,
    pub category_id: Option<String>,
    pub transaction_kind: String,
    pub is_reviewed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionQuery {
    pub account_id: Option<String>,
    pub search: Option<String>,
    pub direction: Option<String>,
    pub category_id: Option<String>,
    pub review_only: bool,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionEdit {
    pub merchant_override: Option<String>,
    pub category_id: Option<String>,
    pub is_reviewed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPage {
    pub items: Vec<TransactionListItem>,
    pub total: usize,
}

pub struct TransactionRepository<'database> {
    database: &'database Database,
}

impl<'database> TransactionRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }
    pub fn page(&self, query: TransactionQuery) -> Result<TransactionPage, DbError> {
        self.database.with_connection(|connection| {
            let mut where_sql = Vec::new(); let mut values = Vec::<rusqlite::types::Value>::new();
            if let Some(account_id) = query.account_id.filter(|v| !v.is_empty()) { where_sql.push("account_id = ?".to_owned()); values.push(account_id.into()); }
            if let Some(search) = query.search.filter(|v| !v.trim().is_empty()) { where_sql.push("(raw_description LIKE ? OR merchant_override LIKE ?)".to_owned()); let like = format!("%{}%", search.trim()); values.push(like.clone().into()); values.push(like.into()); }
            if let Some(direction) = query.direction.filter(|v| !v.is_empty()) { where_sql.push("direction = ?".to_owned()); values.push(direction.into()); }
            if let Some(category_id) = query.category_id.filter(|v| !v.is_empty()) { where_sql.push("category_id = ?".to_owned()); values.push(category_id.into()); }
            if query.review_only { where_sql.push("is_reviewed = 0".to_owned()); }
            let clause = if where_sql.is_empty() { String::new() } else { format!(" WHERE {}", where_sql.join(" AND ")) };
            let total: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM transactions{clause}"), rusqlite::params_from_iter(values.iter()), |row| row.get(0))?;
            values.push((query.limit.clamp(1, 200) as i64).into()); values.push((query.offset as i64).into());
            let sql = format!("SELECT id, transaction_date, direction, amount_paise, raw_description, merchant_override, category_id, transaction_kind, is_reviewed FROM transactions{clause} ORDER BY transaction_date DESC, created_at DESC LIMIT ? OFFSET ?");
            let mut statement = connection.prepare(&sql)?;
            let items = statement.query_map(rusqlite::params_from_iter(values.iter()), |row| Ok(TransactionListItem { id: row.get(0)?, transaction_date: row.get(1)?, direction: row.get(2)?, amount_paise: row.get(3)?, raw_description: row.get(4)?, merchant_override: row.get(5)?, category_id: row.get(6)?, transaction_kind: row.get(7)?, is_reviewed: row.get::<_, i64>(8)? == 1 }))?.collect::<Result<Vec<_>, _>>()?;
            Ok(TransactionPage { items, total: total as usize })
        })
    }
    pub fn edit(&self, id: &str, edit: TransactionEdit) -> Result<(), DbError> {
        self.database.with_connection(|connection| { let count = connection.execute("UPDATE transactions SET merchant_override = ?1, category_id = ?2, is_reviewed = ?3, classification_source = 'user', updated_at = ?4 WHERE id = ?5", params![optional_text(edit.merchant_override), edit.category_id, edit.is_reviewed as i64, utc_timestamp(), id])?; if count == 0 { return Err(DbError::NotFound); } Ok(()) })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDraft {
    pub name: String,
    pub institution: Option<String>,
    pub account_label: Option<String>,
}

pub struct AccountRepository<'database> {
    database: &'database Database,
}

impl<'database> AccountRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn create(&self, draft: AccountDraft) -> Result<Account, DbError> {
        let name = required_text(draft.name, "account name")?;
        let institution =
            optional_text(draft.institution).unwrap_or_else(|| "Canara Bank".to_owned());
        let account_label = optional_text(draft.account_label);
        let account = Account {
            id: Uuid::new_v4().to_string(),
            name,
            institution,
            account_label,
            currency: "INR".to_owned(),
            is_archived: false,
            created_at: utc_timestamp(),
            updated_at: utc_timestamp(),
        };

        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO accounts (
                    id, name, institution, account_label, currency, is_archived, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
                params![
                    account.id,
                    account.name,
                    account.institution,
                    account.account_label,
                    account.currency,
                    account.created_at,
                    account.updated_at,
                ],
            )?;
            Ok(account)
        })
    }

    pub fn list_active(&self) -> Result<Vec<Account>, DbError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, name, institution, account_label, currency, is_archived, created_at, updated_at
                 FROM accounts WHERE is_archived = 0 ORDER BY created_at, id",
            )?;
            let accounts = statement
                .query_map([], account_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(accounts)
        })
    }

    pub fn rename(&self, id: &str, name: String) -> Result<(), DbError> {
        let name = required_text(name, "account name")?;
        self.database.with_connection(|connection| {
            let rows_changed = connection.execute(
                "UPDATE accounts SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![name, utc_timestamp(), id],
            )?;
            if rows_changed == 0 {
                return Err(DbError::NotFound);
            }
            Ok(())
        })
    }

    pub fn archive(&self, id: &str) -> Result<(), DbError> {
        self.database.with_connection(|connection| {
            let rows_changed = connection.execute(
                "UPDATE accounts SET is_archived = 1, updated_at = ?1 WHERE id = ?2",
                params![utc_timestamp(), id],
            )?;
            if rows_changed == 0 {
                return Err(DbError::NotFound);
            }
            Ok(())
        })
    }
}

fn account_from_row(row: &Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        institution: row.get(2)?,
        account_label: row.get(3)?,
        currency: row.get(4)?,
        is_archived: row.get::<_, i64>(5)? == 1,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn required_text(value: String, field: &str) -> Result<String, DbError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DbError::Validation(format!("{field} is required")));
    }
    Ok(value.to_owned())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{AccountDraft, AccountRepository};
    use crate::db::{Database, DbError};

    #[test]
    fn account_setup_uses_inr_and_canara_defaults_then_archives_cleanly() {
        let database = Database::open_in_memory().expect("synthetic database");
        let repository = AccountRepository::new(&database);
        let account = repository
            .create(AccountDraft {
                name: "  Daily account  ".into(),
                institution: None,
                account_label: Some("  Main  ".into()),
            })
            .expect("account setup succeeds");

        assert!(Uuid::parse_str(&account.id).is_ok());
        assert_eq!(account.name, "Daily account");
        assert_eq!(account.institution, "Canara Bank");
        assert_eq!(account.account_label.as_deref(), Some("Main"));
        assert_eq!(account.currency, "INR");

        repository
            .rename(&account.id, "Primary account".into())
            .expect("rename succeeds");
        assert_eq!(
            repository.list_active().expect("active accounts")[0].name,
            "Primary account"
        );

        repository.archive(&account.id).expect("archive succeeds");
        assert!(repository
            .list_active()
            .expect("active accounts")
            .is_empty());
    }

    #[test]
    fn account_setup_rejects_blank_names_and_missing_records() {
        let database = Database::open_in_memory().expect("synthetic database");
        let repository = AccountRepository::new(&database);

        let blank = repository.create(AccountDraft {
            name: "  ".into(),
            institution: None,
            account_label: None,
        });
        assert!(matches!(blank, Err(DbError::Validation(_))));
        assert!(matches!(
            repository.archive("missing"),
            Err(DbError::NotFound)
        ));
    }
}
