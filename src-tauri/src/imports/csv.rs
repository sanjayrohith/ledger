use std::{collections::HashSet, fmt, fs, path::Path};

use csv::{ReaderBuilder, StringRecord};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{format_description, Date};
use uuid::Uuid;

use crate::{
    db::{Database, DbError},
    domain::transaction::{parse_positive_paise, utc_timestamp, Direction, MoneyParseError},
};

#[derive(Debug)]
pub enum ImportError {
    Csv(csv::Error),
    Database(DbError),
    Io(std::io::Error),
    InvalidMapping(String),
    InvalidPath(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv(error) => write!(formatter, "could not read CSV: {error}"),
            Self::Database(error) => write!(formatter, "database operation failed: {error}"),
            Self::Io(error) => write!(formatter, "could not read selected file: {error}"),
            Self::InvalidMapping(message) => formatter.write_str(message),
            Self::InvalidPath(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<DbError> for ImportError {
    fn from(error: DbError) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for ImportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for ImportError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CsvMapping {
    pub transaction_date: String,
    pub description: String,
    pub debit: Option<String>,
    pub credit: Option<String>,
    pub amount: Option<String>,
    pub direction: Option<String>,
    pub value_date: Option<String>,
    pub balance: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RowIssue {
    pub row_number: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCandidate {
    pub row_number: usize,
    pub kind: DuplicateKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateKind {
    TrustedReference,
    LikelyFingerprint,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    pub original_filename: String,
    pub headers: Vec<String>,
    pub mapping: CsvMapping,
    pub rows_seen: usize,
    pub valid_rows: usize,
    pub invalid_rows: Vec<RowIssue>,
    pub duplicate_candidates: Vec<DuplicateCandidate>,
    pub debit_total_paise: i64,
    pub credit_total_paise: i64,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub batch_id: String,
    pub rows_seen: usize,
    pub rows_imported: usize,
    pub rows_rejected: usize,
    pub rows_skipped_duplicate: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedRow {
    pub(crate) row_number: usize,
    pub(crate) transaction_date: String,
    pub(crate) value_date: Option<String>,
    pub(crate) raw_description: String,
    pub(crate) normalized_description: String,
    pub(crate) amount_paise: i64,
    pub(crate) direction: Direction,
    pub(crate) balance_after_paise: Option<i64>,
    pub(crate) reference_raw: Option<String>,
    pub(crate) fingerprint: String,
}

#[derive(Debug)]
pub(crate) struct ParsedCsv {
    pub(crate) filename: String,
    pub(crate) sha256: String,
    pub(crate) source_type: &'static str,
    pub(crate) headers: Vec<String>,
    pub(crate) mapping: CsvMapping,
    pub(crate) rows_seen: usize,
    pub(crate) valid_rows: Vec<ParsedRow>,
    pub(crate) invalid_rows: Vec<RowIssue>,
}

pub struct CsvImportService<'database> {
    database: &'database Database,
}

impl<'database> CsvImportService<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn preview(
        &self,
        account_id: &str,
        path: &str,
        mapping: Option<CsvMapping>,
    ) -> Result<CsvPreview, ImportError> {
        self.preview_parsed(account_id, parse_csv(path, mapping)?)
    }

    pub fn commit(
        &self,
        account_id: &str,
        path: &str,
        mapping: CsvMapping,
    ) -> Result<CommitResult, ImportError> {
        self.commit_parsed(account_id, parse_csv(path, Some(mapping))?)
    }

    pub(crate) fn preview_parsed(
        &self,
        account_id: &str,
        parsed: ParsedCsv,
    ) -> Result<CsvPreview, ImportError> {
        let duplicates = self.find_duplicates(account_id, &parsed.valid_rows)?;
        Ok(build_preview(parsed, duplicates))
    }

    pub(crate) fn commit_parsed(
        &self,
        account_id: &str,
        parsed: ParsedCsv,
    ) -> Result<CommitResult, ImportError> {
        let batch_id = Uuid::new_v4().to_string();
        let imported_at = utc_timestamp();
        let period_start = parsed
            .valid_rows
            .iter()
            .map(|row| row.transaction_date.as_str())
            .min();
        let period_end = parsed
            .valid_rows
            .iter()
            .map(|row| row.transaction_date.as_str())
            .max();

        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO import_batches (
                    id, account_id, source_type, original_filename, file_sha256, imported_at,
                    period_start, period_end, rows_seen, rows_rejected, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'committed')",
                params![
                    batch_id,
                    account_id,
                    parsed.source_type,
                    parsed.filename,
                    parsed.sha256,
                    imported_at,
                    period_start,
                    period_end,
                    parsed.rows_seen as i64,
                    parsed.invalid_rows.len() as i64,
                ],
            )?;

            let mut rows_imported = 0usize;
            let mut rows_skipped_duplicate = 0usize;
            let mut seen_references = HashSet::new();
            for row in &parsed.valid_rows {
                let is_duplicate = trusted_reference_exists(&transaction, account_id, row)?
                    || row
                        .reference_raw
                        .as_ref()
                        .is_some_and(|reference| !seen_references.insert(reference.to_uppercase()));
                if is_duplicate {
                    rows_skipped_duplicate += 1;
                    continue;
                }

                transaction.execute(
                    "INSERT INTO transactions (
                        id, account_id, import_batch_id, transaction_date, value_date, direction,
                        amount_paise, balance_after_paise, raw_description, normalized_description,
                        reference_raw, transaction_kind, source_row_number, fingerprint, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'unknown', ?12, ?13, ?14, ?14)",
                    params![
                        Uuid::new_v4().to_string(),
                        account_id,
                        batch_id,
                        row.transaction_date,
                        row.value_date,
                        direction_text(row.direction),
                        row.amount_paise,
                        row.balance_after_paise,
                        row.raw_description,
                        row.normalized_description,
                        row.reference_raw,
                        row.row_number as i64,
                        row.fingerprint,
                        imported_at,
                    ],
                )?;
                rows_imported += 1;
            }

            transaction.execute(
                "UPDATE import_batches
                 SET rows_imported = ?1, rows_skipped_duplicate = ?2
                 WHERE id = ?3",
                params![rows_imported as i64, rows_skipped_duplicate as i64, batch_id],
            )?;
            transaction.commit()?;
            Ok(CommitResult {
                batch_id,
                rows_seen: parsed.rows_seen,
                rows_imported,
                rows_rejected: parsed.invalid_rows.len(),
                rows_skipped_duplicate,
            })
        })
        .map_err(ImportError::from)
    }

    fn find_duplicates(
        &self,
        account_id: &str,
        rows: &[ParsedRow],
    ) -> Result<Vec<DuplicateCandidate>, ImportError> {
        self.database
            .with_connection(|connection| {
                let mut candidates = Vec::new();
                let mut seen_references = HashSet::new();
                for row in rows {
                    let trusted = trusted_reference_exists(connection, account_id, row)?
                        || row.reference_raw.as_ref().is_some_and(|reference| {
                            !seen_references.insert(reference.to_uppercase())
                        });
                    if trusted {
                        candidates.push(DuplicateCandidate {
                            row_number: row.row_number,
                            kind: DuplicateKind::TrustedReference,
                        });
                    } else if fingerprint_exists(connection, account_id, &row.fingerprint)? {
                        candidates.push(DuplicateCandidate {
                            row_number: row.row_number,
                            kind: DuplicateKind::LikelyFingerprint,
                        });
                    }
                }
                Ok(candidates)
            })
            .map_err(ImportError::from)
    }
}

fn parse_csv(path: &str, mapping: Option<CsvMapping>) -> Result<ParsedCsv, ImportError> {
    let path = Path::new(path);
    if path.extension().and_then(|extension| extension.to_str()) != Some("csv") {
        return Err(ImportError::InvalidPath(
            "select a .csv statement file".to_owned(),
        ));
    }
    let bytes = fs::read(path)?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ImportError::InvalidPath("selected file has no valid filename".to_owned()))?
        .to_owned();
    parse_csv_bytes(&bytes, filename, sha256_hex(&bytes), "canara_csv", mapping)
}

pub(crate) fn parse_csv_bytes(
    bytes: &[u8],
    filename: String,
    sha256: String,
    source_type: &'static str,
    mapping: Option<CsvMapping>,
) -> Result<ParsedCsv, ImportError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ImportError::InvalidPath(
            "CSV must be UTF-8 encoded; no file contents were imported".to_owned(),
        )
    })?;
    let delimiter = detect_delimiter(text);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    let header_values = headers.iter().map(str::to_owned).collect::<Vec<_>>();
    let mapping = mapping.unwrap_or_else(|| propose_mapping(&headers));
    validate_mapping(&headers, &mapping)?;

    let mut valid_rows = Vec::new();
    let mut invalid_rows = Vec::new();
    let mut rows_seen = 0usize;
    for (index, record) in reader.records().enumerate() {
        let row_number = index + 2;
        rows_seen += 1;
        let parsed_row = match record {
            Ok(record) => parse_row(&headers, &record, &mapping),
            Err(error) => Err(ImportError::Csv(error)),
        };
        match parsed_row {
            Ok(row) => valid_rows.push(ParsedRow { row_number, ..row }),
            Err(error) => invalid_rows.push(RowIssue {
                row_number,
                reason: error.to_string(),
            }),
        }
    }

    Ok(ParsedCsv {
        filename,
        sha256,
        source_type,
        headers: header_values,
        mapping,
        rows_seen,
        valid_rows,
        invalid_rows,
    })
}

fn build_preview(parsed: ParsedCsv, duplicate_candidates: Vec<DuplicateCandidate>) -> CsvPreview {
    let debit_total_paise = parsed
        .valid_rows
        .iter()
        .filter(|row| row.direction == Direction::Debit)
        .map(|row| row.amount_paise)
        .sum();
    let credit_total_paise = parsed
        .valid_rows
        .iter()
        .filter(|row| row.direction == Direction::Credit)
        .map(|row| row.amount_paise)
        .sum();
    let period_start = parsed
        .valid_rows
        .iter()
        .map(|row| row.transaction_date.clone())
        .min();
    let period_end = parsed
        .valid_rows
        .iter()
        .map(|row| row.transaction_date.clone())
        .max();
    CsvPreview {
        original_filename: parsed.filename,
        headers: parsed.headers,
        mapping: parsed.mapping,
        rows_seen: parsed.rows_seen,
        valid_rows: parsed.valid_rows.len(),
        invalid_rows: parsed.invalid_rows,
        duplicate_candidates,
        debit_total_paise,
        credit_total_paise,
        period_start,
        period_end,
    }
}

fn detect_delimiter(input: &str) -> u8 {
    let sample = input.lines().take(4).collect::<Vec<_>>().join("\n");
    [
        (b',', sample.matches(',').count()),
        (b';', sample.matches(';').count()),
        (b'\t', sample.matches('\t').count()),
    ]
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .map(|(delimiter, _)| delimiter)
    .unwrap_or(b',')
}

fn propose_mapping(headers: &StringRecord) -> CsvMapping {
    CsvMapping {
        transaction_date: find_header(headers, &["transactiondate", "txn date", "date"])
            .unwrap_or_default(),
        description: find_header(headers, &["narration", "description", "particulars"])
            .unwrap_or_default(),
        debit: find_header(headers, &["withdrawal", "debit", "dr"]),
        credit: find_header(headers, &["deposit", "credit", "cr"]),
        amount: find_header(headers, &["amount"]),
        direction: find_header(headers, &["direction", "drcr", "debitcredit"]),
        value_date: find_header(headers, &["valuedate"]),
        balance: find_header(headers, &["balance"]),
        reference: find_header(headers, &["reference", "utr", "cheque"]),
    }
}

fn find_header(headers: &StringRecord, candidates: &[&str]) -> Option<String> {
    headers.iter().find_map(|header| {
        let normalized = normalize_header(header);
        candidates
            .iter()
            .any(|candidate| normalized.contains(&normalize_header(candidate)))
            .then(|| header.to_owned())
    })
}

fn validate_mapping(headers: &StringRecord, mapping: &CsvMapping) -> Result<(), ImportError> {
    for required in [&mapping.transaction_date, &mapping.description] {
        if required.is_empty() || !headers.iter().any(|header| header == required) {
            return Err(ImportError::InvalidMapping(
                "map transaction date and description columns".to_owned(),
            ));
        }
    }
    let has_separate_amounts = mapping.debit.is_some() || mapping.credit.is_some();
    let has_amount_and_direction = mapping.amount.is_some() && mapping.direction.is_some();
    if !has_separate_amounts && !has_amount_and_direction {
        return Err(ImportError::InvalidMapping(
            "map debit/credit columns or an amount column with a direction column".to_owned(),
        ));
    }
    for column in optional_columns(mapping) {
        if !headers.iter().any(|header| header == column) {
            return Err(ImportError::InvalidMapping(format!(
                "mapped column is missing: {column}"
            )));
        }
    }
    Ok(())
}

fn optional_columns(mapping: &CsvMapping) -> Vec<&str> {
    [
        Some(mapping.transaction_date.as_str()),
        Some(mapping.description.as_str()),
        mapping.debit.as_deref(),
        mapping.credit.as_deref(),
        mapping.amount.as_deref(),
        mapping.direction.as_deref(),
        mapping.value_date.as_deref(),
        mapping.balance.as_deref(),
        mapping.reference.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn parse_row(
    headers: &StringRecord,
    record: &StringRecord,
    mapping: &CsvMapping,
) -> Result<ParsedRow, ImportError> {
    let transaction_date =
        parse_statement_date(value(headers, record, &mapping.transaction_date)?)?.to_string();
    let value_date = mapped_optional(headers, record, mapping.value_date.as_deref())?
        .map(|value| parse_statement_date(&value).map(|date| date.to_string()))
        .transpose()?;
    let raw_description = value(headers, record, &mapping.description)?
        .trim()
        .to_owned();
    if raw_description.is_empty() {
        return Err(ImportError::InvalidMapping(
            "description is empty".to_owned(),
        ));
    }
    let (amount_paise, direction) = parse_amount_and_direction(headers, record, mapping)?;
    let balance_after_paise = mapped_optional(headers, record, mapping.balance.as_deref())?
        .map(|balance| parse_signed_paise(&balance))
        .transpose()?;
    let reference_raw = mapped_optional(headers, record, mapping.reference.as_deref())?;
    let normalized_description = normalize_description(&raw_description);
    let fingerprint = fingerprint(
        &transaction_date,
        direction,
        amount_paise,
        &normalized_description,
        reference_raw.as_deref(),
    );
    Ok(ParsedRow {
        row_number: 0,
        transaction_date,
        value_date,
        raw_description,
        normalized_description,
        amount_paise,
        direction,
        balance_after_paise,
        reference_raw,
        fingerprint,
    })
}

fn parse_amount_and_direction(
    headers: &StringRecord,
    record: &StringRecord,
    mapping: &CsvMapping,
) -> Result<(i64, Direction), ImportError> {
    let debit = mapping
        .debit
        .as_deref()
        .map(|column| optional_value(headers, record, column))
        .transpose()?;
    let credit = mapping
        .credit
        .as_deref()
        .map(|column| optional_value(headers, record, column))
        .transpose()?;
    match (debit.flatten(), credit.flatten()) {
        (Some(_), Some(_)) => Err(ImportError::InvalidMapping(
            "both debit and credit amounts are present".to_owned(),
        )),
        (Some(amount), None) => Ok((parse_money(&amount)?, Direction::Debit)),
        (None, Some(amount)) => Ok((parse_money(&amount)?, Direction::Credit)),
        (None, None) => {
            let amount_column = mapping.amount.as_deref().ok_or_else(|| {
                ImportError::InvalidMapping("amount mapping is missing".to_owned())
            })?;
            let direction_column = mapping.direction.as_deref().ok_or_else(|| {
                ImportError::InvalidMapping("direction mapping is missing".to_owned())
            })?;
            let amount = value(headers, record, amount_column)?;
            let direction = match value(headers, record, direction_column)?
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "debit" | "dr" | "withdrawal" => Direction::Debit,
                "credit" | "cr" | "deposit" => Direction::Credit,
                _ => {
                    return Err(ImportError::InvalidMapping(
                        "direction must be debit/credit or dr/cr".to_owned(),
                    ))
                }
            };
            Ok((parse_money(amount)?, direction))
        }
    }
}

fn value<'a>(
    headers: &StringRecord,
    record: &'a StringRecord,
    column: &str,
) -> Result<&'a str, ImportError> {
    let index = headers
        .iter()
        .position(|header| header == column)
        .ok_or_else(|| {
            ImportError::InvalidMapping(format!("mapped column is missing: {column}"))
        })?;
    record
        .get(index)
        .ok_or_else(|| ImportError::InvalidMapping(format!("missing value for column: {column}")))
}

fn optional_value(
    headers: &StringRecord,
    record: &StringRecord,
    column: &str,
) -> Result<Option<String>, ImportError> {
    let value = value(headers, record, column)?.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

fn mapped_optional(
    headers: &StringRecord,
    record: &StringRecord,
    column: Option<&str>,
) -> Result<Option<String>, ImportError> {
    column.map_or(Ok(None), |name| optional_value(headers, record, name))
}

fn parse_money(input: &str) -> Result<i64, ImportError> {
    parse_positive_paise(input)
        .map_err(|error| ImportError::InvalidMapping(money_reason(error).to_owned()))
}

fn parse_signed_paise(input: &str) -> Result<i64, ImportError> {
    let input = input.trim();
    if let Some(value) = input.strip_prefix('-') {
        return parse_positive_paise(value)
            .map(|paise| -paise)
            .map_err(|error| ImportError::InvalidMapping(money_reason(error).to_owned()));
    }
    if input == "0" || input == "0.00" {
        return Ok(0);
    }
    parse_money(input)
}

fn money_reason(error: MoneyParseError) -> &'static str {
    match error {
        MoneyParseError::Empty => "amount is empty",
        MoneyParseError::Invalid => "amount is malformed",
        MoneyParseError::TooManyFractionalDigits => "amount has more than two decimal places",
        MoneyParseError::Overflow => "amount is too large",
        MoneyParseError::ZeroOrNegative => "amount must be greater than zero",
    }
}

fn parse_statement_date(input: &str) -> Result<Date, ImportError> {
    let formats = [
        "[year]-[month]-[day]",
        "[day]/[month]/[year]",
        "[day]-[month]-[year]",
    ];
    formats
        .iter()
        .find_map(|format| {
            format_description::parse_borrowed::<2>(format)
                .ok()
                .and_then(|description| Date::parse(input.trim(), &description).ok())
        })
        .ok_or_else(|| ImportError::InvalidMapping("transaction date is unparseable".to_owned()))
}

fn normalize_header(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn normalize_description(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn fingerprint(
    date: &str,
    direction: Direction,
    amount_paise: i64,
    description: &str,
    reference: Option<&str>,
) -> String {
    let direction = direction_text(direction);
    let source = format!(
        "{date}|{direction}|{amount_paise}|{description}|{}",
        reference.unwrap_or_default().trim().to_ascii_uppercase()
    );
    sha256_hex(source.as_bytes())
}

fn sha256_hex(input: &[u8]) -> String {
    Sha256::digest(input)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Debit => "debit",
        Direction::Credit => "credit",
    }
}

fn trusted_reference_exists(
    connection: &rusqlite::Connection,
    account_id: &str,
    row: &ParsedRow,
) -> Result<bool, DbError> {
    let Some(reference) = row.reference_raw.as_deref() else {
        return Ok(false);
    };
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id = ?1 AND reference_raw = ?2)",
        params![account_id, reference],
        |result| result.get(0),
    ).map_err(DbError::from)
}

fn fingerprint_exists(
    connection: &rusqlite::Connection,
    account_id: &str,
    fingerprint: &str,
) -> Result<bool, DbError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM transactions WHERE account_id = ?1 AND fingerprint = ?2)",
            params![account_id, fingerprint],
            |result| result.get(0),
        )
        .map_err(DbError::from)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::CsvImportService;
    use crate::db::{
        repositories::{AccountDraft, AccountRepository},
        Database,
    };

    fn write_fixture(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ledger-synthetic-{}.csv", Uuid::new_v4()));
        fs::write(&path, contents).expect("synthetic fixture write");
        path
    }

    fn account_id(database: &Database) -> String {
        AccountRepository::new(database)
            .create(AccountDraft {
                name: "Synthetic Canara account".into(),
                institution: None,
                account_label: None,
            })
            .expect("synthetic account")
            .id
    }

    #[test]
    fn preview_reports_invalid_rows_totals_and_dates_without_exposing_row_contents() {
        let database = Database::open_in_memory().expect("synthetic database");
        let account_id = account_id(&database);
        let path = write_fixture(
            "Transaction Date,Narration,Withdrawal Amt.,Deposit Amt.,UTR\n\
             2026-01-10,SYNTHETIC MARKET,125.50,,SYN-001\n\
             11/01/2026,SYNTHETIC SALARY,,550.00,SYN-002\n\
             invalid,SYNTHETIC BAD,9.00,,SYN-003\n\
             2026-01-12,SYNTHETIC CONFLICT,1.00,1.00,SYN-004\n",
        );

        let preview = CsvImportService::new(&database)
            .preview(&account_id, path.to_str().expect("utf-8 path"), None)
            .expect("preview succeeds");

        assert_eq!(preview.rows_seen, 4);
        assert_eq!(preview.valid_rows, 2);
        assert_eq!(preview.invalid_rows.len(), 2);
        assert_eq!(preview.debit_total_paise, 12_550);
        assert_eq!(preview.credit_total_paise, 55_000);
        assert_eq!(preview.period_start.as_deref(), Some("2026-01-10"));
        assert_eq!(preview.period_end.as_deref(), Some("2026-01-11"));
        assert!(preview
            .invalid_rows
            .iter()
            .all(|issue| !issue.reason.contains("SYNTHETIC")));

        fs::remove_file(path).expect("fixture cleanup");
    }

    #[test]
    fn repeat_import_skips_trusted_references_without_double_counting() {
        let database = Database::open_in_memory().expect("synthetic database");
        let account_id = account_id(&database);
        let path = write_fixture(
            "Transaction Date,Narration,Withdrawal Amt.,Deposit Amt.,UTR\n\
             2026-01-10,SYNTHETIC MARKET,125.50,,SYN-001\n\
             2026-01-11,SYNTHETIC SALARY,,550.00,SYN-002\n",
        );
        let service = CsvImportService::new(&database);
        let preview = service
            .preview(&account_id, path.to_str().expect("utf-8 path"), None)
            .expect("preview succeeds");
        let first = service
            .commit(
                &account_id,
                path.to_str().expect("utf-8 path"),
                preview.mapping.clone(),
            )
            .expect("first commit succeeds");
        let repeat_preview = service
            .preview(
                &account_id,
                path.to_str().expect("utf-8 path"),
                Some(preview.mapping.clone()),
            )
            .expect("repeat preview succeeds");
        let second = service
            .commit(
                &account_id,
                path.to_str().expect("utf-8 path"),
                preview.mapping,
            )
            .expect("repeat commit succeeds");

        assert_eq!(first.rows_imported, 2);
        assert_eq!(repeat_preview.duplicate_candidates.len(), 2);
        assert_eq!(second.rows_imported, 0);
        assert_eq!(second.rows_skipped_duplicate, 2);
        let transaction_count: i64 = database
            .with_connection(|connection| {
                Ok(connection
                    .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?)
            })
            .expect("transaction count");
        assert_eq!(transaction_count, 2);

        fs::remove_file(path).expect("fixture cleanup");
    }

    #[test]
    fn failed_commit_rolls_back_batch_and_transaction_rows() {
        let database = Database::open_in_memory().expect("synthetic database");
        let path = write_fixture(
            "Transaction Date,Narration,Withdrawal Amt.,Deposit Amt.,UTR\n\
             2026-01-10,SYNTHETIC MARKET,125.50,,SYN-001\n",
        );
        let service = CsvImportService::new(&database);
        let mapping = service
            .preview("missing-account", path.to_str().expect("utf-8 path"), None)
            .expect("preview does not persist")
            .mapping;

        assert!(service
            .commit(
                "missing-account",
                path.to_str().expect("utf-8 path"),
                mapping
            )
            .is_err());
        let (batch_count, transaction_count): (i64, i64) = database
            .with_connection(|connection| {
                Ok((
                    connection
                        .query_row("SELECT COUNT(*) FROM import_batches", [], |row| row.get(0))?,
                    connection
                        .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?,
                ))
            })
            .expect("rollback counts");
        assert_eq!((batch_count, transaction_count), (0, 0));

        fs::remove_file(path).expect("fixture cleanup");
    }
}
