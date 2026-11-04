use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Database, DbError};
use crate::domain::rules::EnabledRule;
use crate::domain::transaction::utc_timestamp;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub kind: String,
}

pub struct CategoryRepository<'database> {
    database: &'database Database,
}
impl<'database> CategoryRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }
    pub fn list_expense(&self) -> Result<Vec<Category>, DbError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare("SELECT id,name,kind FROM categories WHERE kind='expense' AND is_archived=0 ORDER BY sort_order,name")?;
            let categories = statement.query_map([], |row| Ok(Category { id: row.get(0)?, name: row.get(1)?, kind: row.get(2)? }))?.collect::<Result<Vec<_>,_>>()?;
            Ok(categories)
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantRule {
    pub id: String,
    pub name: String,
    pub match_type: String,
    pub pattern: String,
    pub merchant_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub transaction_kind: Option<String>,
    pub priority: i64,
    pub is_enabled: bool,
    pub explanation: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantRuleDraft {
    pub name: String,
    pub match_type: String,
    pub pattern: String,
    pub merchant_name: Option<String>,
    pub category_id: Option<String>,
    pub transaction_kind: Option<String>,
    pub priority: i64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantRuleUpdate {
    pub draft: MerchantRuleDraft,
    pub is_enabled: bool,
}
pub struct RuleRepository<'database> {
    database: &'database Database,
}
impl<'database> RuleRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }
    pub fn list(&self) -> Result<Vec<MerchantRule>, DbError> {
        self.database.with_connection(list_rules)
    }
    pub fn create(&self, draft: MerchantRuleDraft) -> Result<MerchantRule, DbError> {
        let name = required_text(draft.name, "rule name")?;
        let pattern = required_text(draft.pattern, "rule pattern")?;
        if !["contains", "equals", "starts_with"].contains(&draft.match_type.as_str()) {
            return Err(DbError::Validation("rule match type is invalid".into()));
        }
        if let Some(kind) = draft.transaction_kind.as_deref() {
            if !["expense", "income", "transfer", "refund", "reversal", "fee"].contains(&kind) {
                return Err(DbError::Validation(
                    "rule transaction kind is invalid".into(),
                ));
            }
        }
        let id = Uuid::new_v4().to_string();
        let now = utc_timestamp();
        self.database.with_connection(|connection| {
            let merchant_id = optional_text(draft.merchant_name).map(|merchant_name| {
                let merchant_id = Uuid::new_v4().to_string();
                connection.execute("INSERT INTO merchants (id,display_name,created_at,updated_at) VALUES (?1,?2,?3,?3) ON CONFLICT(display_name) DO UPDATE SET updated_at=excluded.updated_at", params![merchant_id,merchant_name,now])?;
                connection.query_row("SELECT id FROM merchants WHERE display_name=?1", params![merchant_name], |row| row.get::<_,String>(0))
            }).transpose()?;
            connection.execute("INSERT INTO merchant_rules (id,name,match_type,pattern,merchant_id,category_id,transaction_kind,priority,is_enabled,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?9)", params![id,name,draft.match_type,pattern,merchant_id,draft.category_id,draft.transaction_kind,draft.priority,now])?;
            list_rules(connection)?.into_iter().find(|rule| rule.id == id).ok_or(DbError::NotFound)
        })
    }
    pub fn update(&self, id: &str, update: MerchantRuleUpdate) -> Result<MerchantRule, DbError> {
        let name = required_text(update.draft.name, "rule name")?;
        let pattern = required_text(update.draft.pattern, "rule pattern")?;
        if !["contains", "equals", "starts_with"].contains(&update.draft.match_type.as_str()) {
            return Err(DbError::Validation("rule match type is invalid".into()));
        }
        let now = utc_timestamp();
        self.database.with_connection(|connection| {
            let merchant_id = optional_text(update.draft.merchant_name).map(|merchant_name| {
                connection.execute("INSERT INTO merchants (id,display_name,created_at,updated_at) VALUES (?1,?2,?3,?3) ON CONFLICT(display_name) DO UPDATE SET updated_at=excluded.updated_at", params![Uuid::new_v4().to_string(),merchant_name,now])?;
                connection.query_row("SELECT id FROM merchants WHERE display_name=?1", params![merchant_name], |row| row.get::<_,String>(0))
            }).transpose()?;
            let changed = connection.execute("UPDATE merchant_rules SET name=?1,match_type=?2,pattern=?3,merchant_id=?4,category_id=?5,transaction_kind=?6,priority=?7,is_enabled=?8,updated_at=?9 WHERE id=?10",params![name,update.draft.match_type,pattern,merchant_id,update.draft.category_id,update.draft.transaction_kind,update.draft.priority,update.is_enabled as i64,now,id])?;
            if changed == 0 { return Err(DbError::NotFound); }
            list_rules(connection)?.into_iter().find(|rule| rule.id == id).ok_or(DbError::NotFound)
        })
    }
    pub fn enabled_for_import(
        connection: &rusqlite::Connection,
    ) -> Result<Vec<EnabledRule>, DbError> {
        let mut statement = connection.prepare("SELECT id,name,match_type,pattern,merchant_id,category_id,transaction_kind,priority FROM merchant_rules WHERE is_enabled=1 ORDER BY priority ASC, created_at ASC, id ASC")?;
        let rules = statement
            .query_map([], |row| {
                Ok(EnabledRule {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    match_type: row.get(2)?,
                    pattern: row.get(3)?,
                    merchant_id: row.get(4)?,
                    category_id: row.get(5)?,
                    transaction_kind: row.get(6)?,
                    priority: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rules)
    }
}
fn list_rules(connection: &rusqlite::Connection) -> Result<Vec<MerchantRule>, DbError> {
    let mut statement = connection.prepare("SELECT r.id,r.name,r.match_type,r.pattern,m.display_name,r.category_id,c.name,r.transaction_kind,r.priority,r.is_enabled FROM merchant_rules r LEFT JOIN merchants m ON m.id=r.merchant_id LEFT JOIN categories c ON c.id=r.category_id ORDER BY r.priority,r.created_at,r.id")?;
    let rules = statement
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let match_type: String = row.get(2)?;
            let pattern: String = row.get(3)?;
            Ok(MerchantRule {
                id: row.get(0)?,
                explanation: format!(
                    "{name}: matched description {} `{pattern}`",
                    match_type.replace('_', " ")
                ),
                name,
                match_type,
                pattern,
                merchant_name: row.get(4)?,
                category_id: row.get(5)?,
                category_name: row.get(6)?,
                transaction_kind: row.get(7)?,
                priority: row.get(8)?,
                is_enabled: row.get::<_, i64>(9)? == 1,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rules)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetStatus {
    pub category_id: String,
    pub category_name: String,
    pub month: String,
    pub amount_paise: i64,
    pub spent_paise: i64,
    pub remaining_paise: i64,
    pub remaining_days: i64,
    pub remaining_per_day_paise: i64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDraft {
    pub category_id: String,
    pub month: String,
    pub amount_paise: i64,
}
pub struct BudgetRepository<'database> {
    database: &'database Database,
}
impl<'database> BudgetRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }
    pub fn upsert(&self, draft: BudgetDraft) -> Result<(), DbError> {
        if draft.amount_paise < 0 {
            return Err(DbError::Validation("budget cannot be negative".into()));
        }
        validate_month(&draft.month)?;
        self.database.with_connection(|connection| { connection.execute("INSERT INTO budgets (id,category_id,month,amount_paise,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?5) ON CONFLICT(category_id,month) DO UPDATE SET amount_paise=excluded.amount_paise,updated_at=excluded.updated_at",params![Uuid::new_v4().to_string(),draft.category_id,draft.month,draft.amount_paise,utc_timestamp()])?; Ok(()) })
    }
    pub fn status(&self, month: &str, today: &str) -> Result<Vec<BudgetStatus>, DbError> {
        validate_month(month)?;
        self.database
            .with_connection(|connection| budget_status(connection, month, today))
    }
}
fn budget_status(
    connection: &rusqlite::Connection,
    month: &str,
    today: &str,
) -> Result<Vec<BudgetStatus>, DbError> {
    let days = remaining_days(month, today)?;
    let next_month = next_month(month)?;
    let mut stmt=connection.prepare("SELECT c.id,c.name,b.amount_paise,COALESCE(SUM(CASE WHEN t.direction='debit' AND t.transaction_kind IN ('expense','fee') THEN t.amount_paise ELSE 0 END),0) FROM budgets b JOIN categories c ON c.id=b.category_id LEFT JOIN transactions t ON t.category_id=c.id AND t.transaction_date>=?2 AND t.transaction_date<?3 WHERE b.month=?1 GROUP BY c.id,c.name,b.amount_paise ORDER BY c.sort_order,c.name")?;
    let status = stmt
        .query_map(params![month, month, next_month], |row| {
            let amount: i64 = row.get(2)?;
            let spent: i64 = row.get(3)?;
            let remaining = amount - spent;
            Ok(BudgetStatus {
                category_id: row.get(0)?,
                category_name: row.get(1)?,
                month: month.into(),
                amount_paise: amount,
                spent_paise: spent,
                remaining_paise: remaining,
                remaining_days: days,
                remaining_per_day_paise: if days > 0 { remaining / days } else { 0 },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(status)
}
fn date_format() -> Result<Vec<time::format_description::BorrowedFormatItem<'static>>, DbError> {
    time::format_description::parse_borrowed::<1>("[year]-[month]-[day]")
        .map_err(|error| DbError::Validation(error.to_string()))
}
fn validate_month(month: &str) -> Result<(), DbError> {
    let format = date_format()?;
    if month.len() != 7
        || month.as_bytes().get(4) != Some(&b'-')
        || time::Date::parse(&format!("{month}-01"), &format).is_err()
    {
        return Err(DbError::Validation("month must be YYYY-MM".into()));
    }
    Ok(())
}
fn next_month(month: &str) -> Result<String, DbError> {
    let format = date_format()?;
    let date = time::Date::parse(&format!("{month}-01"), &format)
        .map_err(|_| DbError::Validation("month must be YYYY-MM".into()))?;
    Ok((date.replace_day(1).unwrap() + time::Duration::days(32))
        .replace_day(1)
        .unwrap()
        .to_string()[..7]
        .into())
}
fn remaining_days(month: &str, today: &str) -> Result<i64, DbError> {
    let format = date_format()?;
    let first = time::Date::parse(&format!("{month}-01"), &format)
        .map_err(|_| DbError::Validation("month must be YYYY-MM".into()))?;
    let current = time::Date::parse(today, &format)
        .map_err(|_| DbError::Validation("today must be YYYY-MM-DD".into()))?;
    let end = (first + time::Duration::days(32)).replace_day(1).unwrap() - time::Duration::days(1);
    if current > end {
        Ok(0)
    } else {
        Ok((end - if current < first { first } else { current }).whole_days() + 1)
    }
}

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewSummary {
    pub period_start: String,
    pub period_end: String,
    pub income_paise: i64,
    pub expense_paise: i64,
    pub net_cash_paise: i64,
    pub previous_income_paise: i64,
    pub previous_expense_paise: i64,
    pub unreviewed_count: i64,
    pub last_imported_at: Option<String>,
    pub coverage_start: Option<String>,
    pub coverage_end: Option<String>,
    pub categories: Vec<(String, i64)>,
    pub trend: Vec<(String, i64)>,
    pub merchants: Vec<(String, i64)>,
    pub budget_alerts: Vec<(String, i64, i64)>,
}

pub struct OverviewRepository<'database> {
    database: &'database Database,
}
impl<'database> OverviewRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }
    pub fn summary(&self, today: &str) -> Result<OverviewSummary, DbError> {
        let month = &today[..7];
        let (year, month_num) = (
            month[..4].parse::<i32>().unwrap_or(2000),
            month[5..7].parse::<u8>().unwrap_or(1),
        );
        let first = time::Date::from_calendar_date(
            year,
            time::Month::try_from(month_num).unwrap_or(time::Month::January),
            1,
        )
        .map_err(|e| DbError::Validation(e.to_string()))?;
        let start = first.to_string();
        let (pm_year, pm_month) = if month_num == 1 {
            (year - 1, 12)
        } else {
            (year, month_num - 1)
        };
        let prev_start =
            time::Date::from_calendar_date(pm_year, time::Month::try_from(pm_month).unwrap(), 1)
                .unwrap()
                .to_string();
        let next_start = if month_num == 12 {
            time::Date::from_calendar_date(year + 1, time::Month::January, 1)
        } else {
            time::Date::from_calendar_date(year, time::Month::try_from(month_num + 1).unwrap(), 1)
        }
        .unwrap()
        .to_string();
        self.database.with_connection(|connection| {
            let totals = |from: &str, to: &str| -> Result<(i64,i64), DbError> { Ok(connection.query_row("SELECT COALESCE(SUM(CASE WHEN direction='credit' AND transaction_kind='income' THEN amount_paise ELSE 0 END),0), COALESCE(SUM(CASE WHEN direction='debit' AND transaction_kind IN ('expense','fee') THEN amount_paise ELSE 0 END),0) FROM transactions WHERE transaction_date >= ?1 AND transaction_date < ?2", params![from,to], |r| Ok((r.get(0)?,r.get(1)?)))?) };
            let (income, expense) = totals(&start, &next_start)?; let (prev_income, prev_expense) = totals(&prev_start, &start)?;
            let unreviewed = connection.query_row("SELECT COUNT(*) FROM transactions WHERE is_reviewed=0", [], |r| r.get(0))?;
            let last = connection.query_row("SELECT MAX(imported_at) FROM import_batches WHERE status='committed'", [], |r| r.get(0))?;
            let coverage = connection.query_row("SELECT MIN(period_start),MAX(period_end) FROM import_batches WHERE status='committed'", [], |r| Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?)))?;
            let mut stmt = connection.prepare("SELECT COALESCE(c.name,'Uncategorised'),SUM(t.amount_paise) FROM transactions t LEFT JOIN categories c ON c.id=t.category_id WHERE t.transaction_date >= ?1 AND t.transaction_date < ?2 AND t.direction='debit' AND t.transaction_kind IN ('expense','fee') GROUP BY COALESCE(c.name,'Uncategorised') ORDER BY SUM(t.amount_paise) DESC")?;
            let categories = stmt.query_map(params![start,next_start], |r| Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            let six_month_start = if month_num <= 5 { time::Date::from_calendar_date(year - 1, time::Month::try_from(month_num + 7).unwrap(), 1).unwrap() } else { time::Date::from_calendar_date(year, time::Month::try_from(month_num - 5).unwrap(), 1).unwrap() }.to_string();
            let mut trend_stmt = connection.prepare("SELECT substr(transaction_date,1,7),SUM(amount_paise) FROM transactions WHERE transaction_date>=?1 AND transaction_date<?2 AND direction='debit' AND transaction_kind IN ('expense','fee') GROUP BY substr(transaction_date,1,7) ORDER BY 1")?;
            let trend = trend_stmt.query_map(params![six_month_start,next_start], |r| Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            let mut merchant_stmt = connection.prepare("SELECT COALESCE(NULLIF(merchant_override,''),raw_description),SUM(amount_paise) FROM transactions WHERE transaction_date>=?1 AND transaction_date<?2 AND direction='debit' AND transaction_kind IN ('expense','fee') GROUP BY COALESCE(NULLIF(merchant_override,''),raw_description) ORDER BY SUM(amount_paise) DESC LIMIT 5")?;
            let merchants = merchant_stmt.query_map(params![start,next_start], |r| Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            let mut alerts_stmt = connection.prepare("SELECT c.name,b.amount_paise,COALESCE(SUM(CASE WHEN t.direction='debit' AND t.transaction_kind IN ('expense','fee') THEN t.amount_paise ELSE 0 END),0) FROM budgets b JOIN categories c ON c.id=b.category_id LEFT JOIN transactions t ON t.category_id=c.id AND t.transaction_date>=?1 AND t.transaction_date<?2 WHERE b.month=?3 GROUP BY c.name,b.amount_paise HAVING COALESCE(SUM(CASE WHEN t.direction='debit' AND t.transaction_kind IN ('expense','fee') THEN t.amount_paise ELSE 0 END),0)>b.amount_paise ORDER BY 3-b.amount_paise DESC")?;
            let budget_alerts = alerts_stmt.query_map(params![start,next_start,month], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<Result<Vec<_>,_>>()?;
            Ok(OverviewSummary { period_start:start, period_end:today.to_owned(), income_paise:income, expense_paise:expense, net_cash_paise:income-expense, previous_income_paise:prev_income, previous_expense_paise:prev_expense, unreviewed_count:unreviewed, last_imported_at:last, coverage_start:coverage.0, coverage_end:coverage.1, categories, trend, merchants, budget_alerts })
        })
    }
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
    use rusqlite::params;
    use uuid::Uuid;

    use super::{
        AccountDraft, AccountRepository, BudgetDraft, BudgetRepository, MerchantRuleDraft,
        MerchantRuleUpdate, OverviewRepository, RuleRepository, TransactionEdit, TransactionQuery,
        TransactionRepository,
    };
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

    #[test]
    fn transaction_pages_filter_and_preserve_user_overrides() {
        let database = Database::open_in_memory().expect("synthetic database");
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Test account".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        database.with_connection(|connection| {
            for n in 0..3 {
                connection.execute("INSERT INTO transactions (id, account_id, transaction_date, direction, amount_paise, raw_description, transaction_kind, is_reviewed, fingerprint, created_at, updated_at) VALUES (?1, ?2, ?3, 'debit', 1250, ?4, 'unknown', ?5, ?6, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')", params![format!("tx-{n}"), account.id, format!("2026-01-0{}", n + 1), format!("Synthetic merchant {n}"), (n == 0) as i64, format!("fp-{n}")])?;
            }
            Ok(())
        }).unwrap();
        let repo = TransactionRepository::new(&database);
        let page = repo
            .page(TransactionQuery {
                account_id: Some(account.id),
                search: Some("merchant 1".into()),
                direction: Some("debit".into()),
                category_id: None,
                review_only: true,
                limit: 1,
                offset: 0,
            })
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].raw_description, "Synthetic merchant 1");
        let id = page.items[0].id.clone();
        repo.edit(
            &id,
            TransactionEdit {
                merchant_override: Some("Corrected shop".into()),
                category_id: None,
                is_reviewed: true,
            },
        )
        .unwrap();
        let edited = repo
            .page(TransactionQuery {
                account_id: None,
                search: Some("Corrected shop".into()),
                direction: None,
                category_id: None,
                review_only: false,
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert_eq!(edited.total, 1);
        assert_eq!(edited.items[0].raw_description, "Synthetic merchant 1");
        assert_eq!(
            edited.items[0].merchant_override.as_deref(),
            Some("Corrected shop")
        );
        assert!(edited.items[0].is_reviewed);
        database
            .with_connection(|connection| {
                let source: String = connection.query_row(
                    "SELECT classification_source FROM transactions WHERE id = ?1",
                    [&id],
                    |row| row.get(0),
                )?;
                assert_eq!(source, "user");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn transaction_page_bounds_result_size() {
        let database = Database::open_in_memory().expect("synthetic database");
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Benchmark account".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        database.with_connection(|connection| {
            let tx = connection.unchecked_transaction()?;
            for n in 0..50_000_i64 {
                tx.execute("INSERT INTO transactions (id, account_id, transaction_date, direction, amount_paise, raw_description, transaction_kind, is_reviewed, fingerprint, created_at, updated_at) VALUES (?1, ?2, '2026-01-01', 'debit', 100, 'Synthetic benchmark', 'unknown', 0, ?3, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')", params![format!("bench-{n}"), account.id, format!("bench-fp-{n}")])?;
            }
            tx.commit()?; Ok(())
        }).unwrap();
        let started = std::time::Instant::now();
        let page = TransactionRepository::new(&database)
            .page(TransactionQuery {
                account_id: Some(account.id),
                search: None,
                direction: None,
                category_id: None,
                review_only: false,
                limit: 50,
                offset: 0,
            })
            .unwrap();
        let elapsed = started.elapsed();
        eprintln!("50,000 synthetic rows: first page query {elapsed:?}");
        assert_eq!(page.total, 50_000);
        assert_eq!(page.items.len(), 50);
        assert!(elapsed.as_millis() < 300, "query took {elapsed:?}");
    }

    #[test]
    fn overview_reconciles_income_expense_transfers_unknown_and_coverage() {
        let database = Database::open_in_memory().expect("synthetic database");
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Synthetic overview".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        database.with_connection(|connection| {
            connection.execute("INSERT INTO import_batches (id,account_id,source_type,original_filename,imported_at,period_start,period_end,status) VALUES ('b1',?1,'canara_csv','synthetic.csv','2026-03-12T00:00:00Z','2026-02-01','2026-03-10','committed')", [&account.id])?;
            for (id,date,direction,amount,kind,category) in [
                ("i1","2026-03-02","credit",10000,"income",None),
                ("e1","2026-03-03","debit",2500,"expense",Some("00000000-0000-4000-8000-000000000001")),
                ("f1","2026-03-04","debit",500,"fee",None),
                ("t1","2026-03-05","debit",3000,"transfer",None),
                ("u1","2026-03-06","debit",4000,"unknown",None),
                ("pe","2026-02-06","debit",1000,"expense",None),
                ("pi","2026-02-07","credit",5000,"income",None),
            ] {
                connection.execute("INSERT INTO transactions (id,account_id,transaction_date,direction,amount_paise,raw_description,transaction_kind,category_id,is_reviewed,fingerprint,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,'Synthetic',?6,?7,0,?8,'2026-03-01T00:00:00Z','2026-03-01T00:00:00Z')", params![id,account.id,date,direction,amount,kind,category,format!("{id}-fp")])?;
            }
            Ok(())
        }).unwrap();
        let overview = OverviewRepository::new(&database)
            .summary("2026-03-15")
            .unwrap();
        assert_eq!(
            (
                overview.income_paise,
                overview.expense_paise,
                overview.net_cash_paise
            ),
            (10000, 3000, 7000)
        );
        assert_eq!(
            (
                overview.previous_income_paise,
                overview.previous_expense_paise
            ),
            (5000, 1000)
        );
        assert_eq!(overview.unreviewed_count, 7);
        assert_eq!(overview.coverage_start.as_deref(), Some("2026-02-01"));
        assert_eq!(overview.coverage_end.as_deref(), Some("2026-03-10"));
        assert_eq!(
            overview.categories,
            vec![
                ("Groceries".to_owned(), 2500),
                ("Uncategorised".to_owned(), 500)
            ]
        );
        assert_eq!(
            overview.trend,
            vec![("2026-02".to_owned(), 1000), ("2026-03".to_owned(), 3000)]
        );
        assert_eq!(overview.merchants, vec![("Synthetic".to_owned(), 3000)]);
        assert!(overview.budget_alerts.is_empty());
    }

    #[test]
    fn budgets_keep_paise_exact_and_handle_over_budget_and_past_months() {
        let database = Database::open_in_memory().unwrap();
        let account = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Budget account".into(),
                institution: None,
                account_label: None,
            })
            .unwrap();
        let groceries = "00000000-0000-4000-8000-000000000001";
        BudgetRepository::new(&database)
            .upsert(BudgetDraft {
                category_id: groceries.into(),
                month: "2026-02".into(),
                amount_paise: 10_000,
            })
            .unwrap();
        database.with_connection(|connection| { connection.execute("INSERT INTO transactions (id,account_id,transaction_date,direction,amount_paise,raw_description,transaction_kind,category_id,is_reviewed,fingerprint,created_at,updated_at) VALUES ('budget-expense',?1,'2026-02-10','debit',12500,'Synthetic','expense',?2,1,'budget-fp','2026-02-10T00:00:00Z','2026-02-10T00:00:00Z')",params![account.id,groceries])?; Ok(()) }).unwrap();
        let current = BudgetRepository::new(&database)
            .status("2026-02", "2026-02-15")
            .unwrap();
        assert_eq!(
            (
                current[0].spent_paise,
                current[0].remaining_paise,
                current[0].remaining_days,
                current[0].remaining_per_day_paise
            ),
            (12_500, -2_500, 14, -178)
        );
        let past = BudgetRepository::new(&database)
            .status("2026-02", "2026-03-01")
            .unwrap();
        assert_eq!(
            (past[0].remaining_days, past[0].remaining_per_day_paise),
            (0, 0)
        );
        assert!(BudgetRepository::new(&database)
            .upsert(BudgetDraft {
                category_id: groceries.into(),
                month: "2026-02".into(),
                amount_paise: -1
            })
            .is_err());
    }

    #[test]
    fn rule_creation_is_explicit_and_ordered() {
        let database = Database::open_in_memory().unwrap();
        let category = "00000000-0000-4000-8000-000000000002";
        let repository = RuleRepository::new(&database);
        let first = repository
            .create(MerchantRuleDraft {
                name: "Later".into(),
                match_type: "contains".into(),
                pattern: "SWIGGY".into(),
                merchant_name: Some("Swiggy".into()),
                category_id: Some(category.into()),
                transaction_kind: Some("expense".into()),
                priority: 20,
            })
            .unwrap();
        repository
            .create(MerchantRuleDraft {
                name: "First".into(),
                match_type: "starts_with".into(),
                pattern: "UPI-".into(),
                merchant_name: None,
                category_id: Some(category.into()),
                transaction_kind: Some("expense".into()),
                priority: 10,
            })
            .unwrap();
        let rules = repository.list().unwrap();
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.name.as_str())
                .collect::<Vec<_>>(),
            vec!["First", "Later"]
        );
        assert_eq!(
            rules[0].explanation,
            "First: matched description starts with `UPI-`"
        );
        let changed = repository
            .update(
                &first.id,
                MerchantRuleUpdate {
                    draft: MerchantRuleDraft {
                        name: "Edited".into(),
                        match_type: "equals".into(),
                        pattern: "UPI-FOOD".into(),
                        merchant_name: None,
                        category_id: Some(category.into()),
                        transaction_kind: Some("expense".into()),
                        priority: 5,
                    },
                    is_enabled: false,
                },
            )
            .unwrap();
        assert_eq!(
            (changed.name, changed.priority, changed.is_enabled),
            ("Edited".into(), 5, false)
        );
    }
}
