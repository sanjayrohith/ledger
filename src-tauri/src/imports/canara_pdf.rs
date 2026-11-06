//! Canara Bank PDF statement import, parsed entirely on this Mac.
//!
//! The layout this reads (columns left to right): Date, Particulars, Deposits, Withdrawals,
//! Balance. Each transaction has one row carrying the date and amounts, surrounded by several
//! wrapped narration lines, and ends with a `Chq:` line. Amounts are right-aligned, so the
//! right edge of an amount says which column it belongs to.
//!
//! Every parsed row is checked against the running balance, and the last balance against the
//! printed closing balance, so a misread row is reported instead of imported silently.

use std::collections::HashMap;
use std::{fs, path::Path};

use sha2::{Digest, Sha256};
use time::{Date, Month};

use crate::db::Database;
use crate::domain::transaction::Direction;
use crate::imports::csv::{
    fingerprint, normalize_description, CommitResult, CsvImportService, CsvMapping, CsvPreview,
    ImportError, ParsedCsv, ParsedRow, RowIssue, StatementCheck,
};
use crate::imports::pdf_layout::{
    char_lines, extract_pages, group_lines, join_chars, PdfChar, PdfError, PdfPage,
};

const MAX_PDF_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfTransaction {
    pub number: usize,
    pub date: String,
    pub narration: String,
    pub reference: Option<String>,
    pub deposit_paise: Option<i64>,
    pub withdrawal_paise: Option<i64>,
    pub balance_paise: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PdfStatement {
    pub transactions: Vec<PdfTransaction>,
    pub opening_paise: Option<i64>,
    pub closing_paise: Option<i64>,
    pub issues: Vec<RowIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The page does not look like a Canara statement table.
    UnsupportedLayout(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLayout(message) => write!(
                formatter,
                "this PDF does not look like a supported Canara statement: {message}"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Columns {
    /// Text left of this x is the date.
    date_end: f64,
    /// Text from here to `particulars_end` is the narration.
    particulars_end: f64,
    withdrawals_start: f64,
    balance_start: f64,
}

fn find_columns(page: &PdfPage) -> Option<(f64, Columns)> {
    for line in group_lines(page) {
        let find = |prefix: &str| {
            line.cells
                .iter()
                .find(|cell| cell.text.trim().to_lowercase().starts_with(prefix))
        };
        let (Some(date), Some(_), Some(deposits), Some(withdrawals), Some(balance)) = (
            find("date"),
            find("particulars"),
            find("deposit"),
            find("withdraw"),
            find("balance"),
        ) else {
            continue;
        };
        if !(deposits.x_start < withdrawals.x_start && withdrawals.x_start < balance.x_start) {
            return None;
        }
        return Some((
            line.y,
            Columns {
                // Dates start 17pt left of the header and are about 62pt wide; narration starts
                // about 80pt right of the header. Cut between them with room on both sides.
                date_end: date.x_start + 54.0,
                particulars_end: deposits.x_start - 15.0,
                withdrawals_start: withdrawals.x_start,
                balance_start: balance.x_start,
            },
        ));
    }
    None
}

/// "1,83,751.50" becomes 18375150. A trailing `Dr` makes the amount negative.
pub fn parse_amount(text: &str) -> Option<i64> {
    let mut cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '₹')
        .collect();
    let mut negative = false;
    let lower = cleaned.to_lowercase();
    if lower.ends_with("dr") {
        negative = true;
        cleaned.truncate(cleaned.len() - 2);
    } else if lower.ends_with("cr") {
        cleaned.truncate(cleaned.len() - 2);
    }
    if let Some(stripped) = cleaned.strip_prefix('-') {
        negative = !negative;
        cleaned = stripped.to_owned();
    }
    let (whole, fraction) = cleaned.split_once('.').unwrap_or((cleaned.as_str(), ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.chars().all(|c| c.is_ascii_digit())
        || !fraction.chars().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
    {
        return None;
    }
    let paise = whole.parse::<i64>().unwrap_or(0).checked_mul(100)?
        + format!("{fraction:0<2}").parse::<i64>().ok()?;
    Some(if negative { -paise } else { paise })
}

/// `01-11-2026`, `01/11/2026` and `01-Nov-2026` become `2026-11-01`.
pub fn parse_date(text: &str) -> Option<String> {
    let parts: Vec<&str> = text.trim().split(['-', '/']).collect();
    let [day, month, year] = parts.as_slice() else {
        return None;
    };
    let day: u8 = day.parse().ok()?;
    let year: i32 = year.parse().ok()?;
    let month = match month.parse::<u8>() {
        Ok(number) => Month::try_from(number).ok()?,
        Err(_) => match month.to_lowercase().get(..3)? {
            "jan" => Month::January,
            "feb" => Month::February,
            "mar" => Month::March,
            "apr" => Month::April,
            "may" => Month::May,
            "jun" => Month::June,
            "jul" => Month::July,
            "aug" => Month::August,
            "sep" => Month::September,
            "oct" => Month::October,
            "nov" => Month::November,
            "dec" => Month::December,
            _ => return None,
        },
    };
    Date::from_calendar_date(year, month, day)
        .ok()
        .map(|date| date.to_string())
}

#[derive(Default)]
struct Block {
    date: Option<String>,
    deposit: Option<i64>,
    withdrawal: Option<i64>,
    balance: Option<i64>,
    lines: Vec<(String, f64)>,
    chq: Option<String>,
}

fn amount_cells(chars: &[&PdfChar]) -> Vec<(f64, String)> {
    let mut cells: Vec<(f64, Vec<&PdfChar>)> = Vec::new();
    for ch in chars {
        match cells.last_mut() {
            Some((end, members)) if ch.x - *end <= ch.size.max(6.0) => {
                *end = end.max(ch.x + ch.width);
                members.push(ch);
            }
            _ => cells.push((ch.x + ch.width, vec![ch])),
        }
    }
    cells
        .into_iter()
        .map(|(end, members)| (end, join_chars(&members)))
        .collect()
}

/// Joins wrapped narration lines. A line that runs to the edge of the column wrapped in the
/// middle of a word, so the next line continues it without a space.
fn join_narration(lines: &[(String, f64)], wrap_edge: f64) -> String {
    let mut narration = String::new();
    let mut previous_full = false;
    for (text, end) in lines {
        if !narration.is_empty() && !previous_full {
            narration.push(' ');
        }
        narration.push_str(text);
        previous_full = *end >= wrap_edge;
    }
    narration
}

fn upi_reference(narration: &str) -> Option<String> {
    let mut parts = narration.split('/');
    let mode = parts.next()?;
    let direction = parts.next()?;
    let reference = parts.next()?;
    (mode.eq_ignore_ascii_case("UPI")
        && (direction.eq_ignore_ascii_case("DR") || direction.eq_ignore_ascii_case("CR"))
        && reference.len() >= 8
        && reference.chars().all(|c| c.is_ascii_digit()))
    .then(|| reference.to_owned())
}

fn chq_reference(text: &str) -> Option<String> {
    let digits: String = text
        .split_once(':')
        .map_or(text, |(_, rest)| rest)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    (digits.len() >= 6 && digits.chars().any(|c| c != '0')).then_some(digits)
}

pub fn parse_statement(pages: &[PdfPage]) -> Result<PdfStatement, ParseError> {
    let mut statement = PdfStatement::default();
    let mut columns: Option<Columns> = None;
    let mut block = Block::default();
    let flush = |block: &mut Block, statement: &mut PdfStatement, wrap_edge: f64| {
        let finished_block = std::mem::take(block);
        let Some(date) = finished_block.date else {
            return;
        };
        let number = statement.transactions.len() + 1;
        let Some(balance) = finished_block.balance else {
            statement.issues.push(RowIssue {
                row_number: number,
                reason: "no balance was found on this row".into(),
            });
            return;
        };
        let (deposit, withdrawal) = (finished_block.deposit, finished_block.withdrawal);
        if deposit.is_some() == withdrawal.is_some()
            || deposit.or(withdrawal).is_some_and(|amount| amount <= 0)
        {
            statement.issues.push(RowIssue {
                row_number: number,
                reason: "expected exactly one positive deposit or withdrawal amount".into(),
            });
            return;
        }
        let narration = join_narration(&finished_block.lines, wrap_edge);
        let reference = upi_reference(&narration)
            .or_else(|| finished_block.chq.as_deref().and_then(chq_reference));
        statement.transactions.push(PdfTransaction {
            number,
            date,
            narration,
            reference,
            deposit_paise: deposit,
            withdrawal_paise: withdrawal,
            balance_paise: balance,
        });
    };

    'pages: for page in pages {
        if let Some((_, found)) = find_columns(page) {
            columns = Some(found);
        }
        let Some(cols) = columns else {
            continue;
        };
        // Anything at or above the table header (account details, the header itself) is not a row.
        let header_y = find_columns(page).map(|(y, _)| y);
        // A line that runs to the right edge of the narration column wrapped inside a word.
        let wrap_edge = cols.particulars_end - 40.0;
        for line in char_lines(page) {
            if header_y.is_some_and(|y| line.y <= y + 1.0) {
                continue;
            }
            let whole = join_chars(&line.chars);
            let lowered = whole.to_lowercase();
            if lowered.starts_with("page ") && whole.split_whitespace().count() == 2 {
                continue;
            }
            if lowered.contains("opening balance") || lowered.contains("closing balance") {
                let money: Vec<&PdfChar> = line
                    .chars
                    .iter()
                    .copied()
                    .filter(|c| c.x >= cols.particulars_end)
                    .collect();
                let balance = amount_cells(&money)
                    .into_iter()
                    .rev()
                    .find_map(|(_, text)| parse_amount(&text));
                if lowered.contains("opening balance") {
                    statement.opening_paise = balance;
                } else {
                    statement.closing_paise = balance;
                    flush(&mut block, &mut statement, wrap_edge);
                    break 'pages;
                }
                continue;
            }
            let date_chars: Vec<&PdfChar> = line
                .chars
                .iter()
                .copied()
                .filter(|c| c.x < cols.date_end)
                .collect();
            let narration_chars: Vec<&PdfChar> = line
                .chars
                .iter()
                .copied()
                .filter(|c| c.x >= cols.date_end && c.x < cols.particulars_end)
                .collect();
            let money_chars: Vec<&PdfChar> = line
                .chars
                .iter()
                .copied()
                .filter(|c| c.x >= cols.particulars_end)
                .collect();
            let date_text = join_chars(&date_chars);
            let narration_text = join_chars(&narration_chars);

            if !date_text.is_empty() {
                if block.date.is_some() {
                    flush(&mut block, &mut statement, wrap_edge);
                }
                match parse_date(&date_text) {
                    Some(date) => block.date = Some(date),
                    None => statement.issues.push(RowIssue {
                        row_number: statement.transactions.len() + 1,
                        reason: "could not read a date in the date column".into(),
                    }),
                }
                for (end, text) in amount_cells(&money_chars) {
                    let Some(value) = parse_amount(&text) else {
                        continue;
                    };
                    if end < cols.withdrawals_start {
                        block.deposit = Some(value);
                    } else if end < cols.balance_start {
                        block.withdrawal = Some(value);
                    } else {
                        block.balance = Some(value);
                    }
                }
            }
            if !narration_text.is_empty() {
                if narration_text.to_lowercase().starts_with("chq") {
                    block.chq = Some(narration_text);
                    flush(&mut block, &mut statement, wrap_edge);
                } else {
                    let end = narration_chars
                        .iter()
                        .map(|c| c.x + c.width)
                        .fold(0.0, f64::max);
                    block.lines.push((narration_text, end));
                }
            }
        }
    }
    if let Some(cols) = columns {
        flush(&mut block, &mut statement, cols.particulars_end - 40.0);
    }

    if columns.is_none() {
        return Err(ParseError::UnsupportedLayout(
            "no Date, Particulars, Deposits, Withdrawals, Balance table was found".into(),
        ));
    }
    Ok(statement)
}

/// Checks each row's balance against the previous one and the final balance against the printed
/// closing balance. Rows that break the chain are returned as issues and left out.
fn validate(statement: &mut PdfStatement) -> (Vec<PdfTransaction>, StatementCheck) {
    let mut accepted = Vec::new();
    let mut previous = statement.opening_paise;
    let mut every_row_balances = true;
    let (mut deposits, mut withdrawals) = (0i64, 0i64);
    for transaction in &statement.transactions {
        let delta =
            transaction.deposit_paise.unwrap_or(0) - transaction.withdrawal_paise.unwrap_or(0);
        match previous {
            Some(before) if before + delta != transaction.balance_paise => {
                every_row_balances = false;
                statement.issues.push(RowIssue {
                    row_number: transaction.number,
                    reason: format!(
                        "balance does not follow from the previous row (expected {}, statement shows {})",
                        rupees(before + delta),
                        rupees(transaction.balance_paise)
                    ),
                });
            }
            _ => {
                deposits += transaction.deposit_paise.unwrap_or(0);
                withdrawals += transaction.withdrawal_paise.unwrap_or(0);
                accepted.push(transaction.clone());
            }
        }
        previous = Some(transaction.balance_paise);
    }
    let totals_match = match (statement.opening_paise, statement.closing_paise) {
        (Some(open), Some(close)) => open + deposits - withdrawals == close && every_row_balances,
        _ => false,
    };
    if statement.closing_paise.is_some() && !totals_match && every_row_balances {
        statement.issues.push(RowIssue {
            row_number: statement.transactions.len(),
            reason: "the rows do not add up to the closing balance printed on the statement; some rows may be missing".into(),
        });
    }
    (
        accepted,
        StatementCheck {
            opening_balance_paise: statement.opening_paise,
            closing_balance_paise: statement.closing_paise,
            deposits_total_paise: deposits,
            withdrawals_total_paise: withdrawals,
            totals_match,
            every_row_balances,
        },
    )
}

fn rupees(paise: i64) -> String {
    format!(
        "{}{}.{:02}",
        if paise < 0 { "-" } else { "" },
        paise.abs() / 100,
        paise.abs() % 100
    )
}

pub(crate) fn to_parsed(
    filename: String,
    sha256: String,
    mut statement: PdfStatement,
) -> ParsedCsv {
    // Issues raised while reading are rows that could not be read at all.
    let unreadable_rows = statement.issues.len();
    let (accepted, check) = validate(&mut statement);
    // A reference that appears twice in one statement is not unique, so it is not trusted.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for transaction in &accepted {
        if let Some(reference) = &transaction.reference {
            *counts.entry(reference.as_str()).or_default() += 1;
        }
    }
    let valid_rows = accepted
        .iter()
        .map(|transaction| {
            let reference = transaction
                .reference
                .clone()
                .filter(|reference| counts.get(reference.as_str()) == Some(&1));
            let (direction, amount) =
                match (transaction.deposit_paise, transaction.withdrawal_paise) {
                    (Some(amount), _) => (Direction::Credit, amount),
                    (_, Some(amount)) => (Direction::Debit, amount),
                    _ => unreachable!("validated rows have exactly one amount"),
                };
            let normalized = normalize_description(&transaction.narration);
            ParsedRow {
                row_number: transaction.number,
                transaction_date: transaction.date.clone(),
                value_date: None,
                fingerprint: fingerprint(
                    &transaction.date,
                    direction,
                    amount,
                    &normalized,
                    reference.as_deref(),
                ),
                raw_description: transaction.narration.clone(),
                normalized_description: normalized,
                amount_paise: amount,
                direction,
                balance_after_paise: Some(transaction.balance_paise),
                reference_raw: reference,
            }
        })
        .collect();
    ParsedCsv {
        filename,
        sha256,
        // The import table only allows a fixed set of source types, so PDFs are recorded as
        // Canara statement imports and identified by the note below.
        source_type: "canara_csv",
        headers: ["Date", "Particulars", "Deposits", "Withdrawals", "Balance"]
            .map(String::from)
            .to_vec(),
        mapping: CsvMapping {
            transaction_date: "Date".into(),
            description: "Particulars".into(),
            debit: Some("Withdrawals".into()),
            credit: Some("Deposits".into()),
            amount: None,
            direction: None,
            value_date: None,
            balance: Some("Balance".into()),
            reference: None,
        },
        rows_seen: statement.transactions.len() + unreadable_rows,
        valid_rows,
        invalid_rows: statement.issues,
        notes: Some("Canara PDF statement"),
        source_kind: "pdf",
        statement_check: Some(check),
    }
}

pub struct PdfImportService<'database> {
    database: &'database Database,
}

impl<'database> PdfImportService<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    fn parse(&self, path: &str, password: Option<&str>) -> Result<ParsedCsv, ImportError> {
        let path = Path::new(path);
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .as_deref()
            != Some("pdf")
        {
            return Err(ImportError::InvalidPath(
                "select a .pdf statement file".into(),
            ));
        }
        if fs::metadata(path)?.len() > MAX_PDF_BYTES {
            return Err(ImportError::InvalidPath(
                "this PDF is larger than 50 MB".into(),
            ));
        }
        let bytes = fs::read(path)?;
        let pages =
            extract_pages(&bytes, password.filter(|value| !value.is_empty())).map_err(|error| {
                match error {
                    PdfError::NeedsPassword => ImportError::PdfPasswordRequired,
                    PdfError::WrongPassword => ImportError::PdfPasswordWrong,
                    other => ImportError::InvalidPath(other.to_string()),
                }
            })?;
        if pages.iter().all(|page| page.chars.is_empty()) {
            return Err(ImportError::InvalidPath(
                "no text was found in this PDF. It may be a scanned image, which Ledger cannot read".into(),
            ));
        }
        let statement =
            parse_statement(&pages).map_err(|error| ImportError::InvalidPath(error.to_string()))?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("statement.pdf")
            .to_owned();
        let sha256 = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(to_parsed(filename, sha256, statement))
    }

    pub fn preview(
        &self,
        account_id: &str,
        path: &str,
        password: Option<&str>,
    ) -> Result<CsvPreview, ImportError> {
        CsvImportService::new(self.database).preview_parsed(account_id, self.parse(path, password)?)
    }

    pub fn commit(
        &self,
        account_id: &str,
        path: &str,
        password: Option<&str>,
    ) -> Result<CommitResult, ImportError> {
        let parsed = self.parse(path, password)?;
        if parsed.valid_rows.is_empty() {
            return Err(ImportError::InvalidPath(
                "no valid transactions were found in this PDF".into(),
            ));
        }
        if parsed.statement_check.as_ref().is_some_and(|check| {
            !check.every_row_balances
                || (check.opening_balance_paise.is_some()
                    && check.closing_balance_paise.is_some()
                    && !check.totals_match)
        }) {
            return Err(ImportError::InvalidPath(
                "the rows do not add up to the statement's balances, so this PDF was not imported. Review the preview first".into(),
            ));
        }
        CsvImportService::new(self.database).commit_parsed(account_id, parsed)
    }
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};

    use super::*;
    use crate::imports::pdf_layout::test_support::{bytes, pdf, width_9pt};

    const PAGE_HEIGHT: f64 = 842.0;

    type Cells = Vec<(f64, String)>;
    type PageLines = Vec<(f64, Cells)>;

    #[derive(Clone)]
    struct Row {
        date: &'static str,
        deposit: bool,
        paise: i64,
        rrn: &'static str,
        merchant: &'static str,
    }

    fn indian(paise: i64) -> String {
        let (whole, fraction) = (paise / 100, paise % 100);
        let digits = whole.to_string();
        let (head, tail) = digits.split_at(digits.len().saturating_sub(3));
        let mut groups = Vec::new();
        let mut head = head.to_owned();
        while head.len() > 2 {
            groups.push(head.split_off(head.len() - 2));
        }
        if !head.is_empty() {
            groups.push(head);
        }
        groups.reverse();
        groups.push(tail.to_owned());
        format!("{}.{fraction:02}", groups.join(","))
    }

    fn right(end: f64, text: &str) -> (f64, String) {
        (end - width_9pt(text), text.to_owned())
    }

    /// A Canara-style statement: two rows per page, a repeated header on later pages, a page
    /// footer, an opening and a closing balance, and trailing legal text.
    fn statement_pdf(
        opening: i64,
        rows: &[Row],
        closing_override: Option<i64>,
        tamper_second_balance: bool,
    ) -> Document {
        let mut pages: Vec<PageLines> = vec![Vec::new()];
        let header = |y: f64| -> (f64, Cells) {
            (
                PAGE_HEIGHT - y,
                vec![
                    (43.5, "Date".into()),
                    (167.0, "Particulars".into()),
                    (320.5, "Deposits".into()),
                    (413.5, "Withdrawals".into()),
                    (517.0, "Balance".into()),
                ],
            )
        };
        pages[0].push((
            PAGE_HEIGHT - 160.5,
            vec![(
                36.0,
                "Statement for Account 123456789012 for the period 01-Nov-2026 to 30-Nov-2026"
                    .into(),
            )],
        ));
        pages[0].push(header(339.5));
        let (o_x, o_text) = right(583.0, &indian(opening));
        pages[0].push((
            PAGE_HEIGHT - 363.5,
            vec![(305.5, "Opening Balance".into()), (o_x, o_text)],
        ));
        let mut balance = opening;
        let mut top = 379.5;
        for (index, row) in rows.iter().enumerate() {
            if index > 0 && index % 2 == 0 {
                pages.push(vec![header(50.0)]);
                top = 74.0;
            }
            balance += if row.deposit { row.paise } else { -row.paise };
            let shown = if tamper_second_balance && index == 1 {
                balance + 100
            } else {
                balance
            };
            let (direction, end) = if row.deposit {
                ("CR", 395.5)
            } else {
                ("DR", 490.5)
            };
            let y = |offset: f64| PAGE_HEIGHT - (top + offset);
            let page = pages.last_mut().unwrap();
            page.push((
                y(0.0),
                vec![(
                    106.5,
                    format!("UPI/{direction}/{}/{}", row.rrn, row.merchant),
                )],
            ));
            page.push((y(12.0), vec![(106.5, "RE/OKAXIS/**1234@ybl/NO".into())]));
            page.push((y(24.0), vec![(106.5, "REMARKS//UPI9X".into())]));
            let (a_x, a_text) = right(end, &indian(row.paise));
            let (b_x, b_text) = right(581.0, &indian(shown));
            page.push((
                y(30.0),
                vec![
                    (26.5, row.date.into()),
                    (106.5, "ABC123/01/11/26".into()),
                    (a_x, a_text),
                    (b_x, b_text),
                ],
            ));
            page.push((y(36.0), vec![(106.5, "2026 14:05:33".into())]));
            page.push((y(60.0), vec![(106.5, format!("Chq: {}", row.rrn))]));
            top += 84.0;
        }
        let closing = closing_override.unwrap_or(balance);
        let (c_x, c_text) = right(583.0, &indian(closing));
        let last = pages.last_mut().unwrap();
        last.push((
            PAGE_HEIGHT - (top + 10.0),
            vec![(311.0, "Closing Balance".into()), (c_x, c_text)],
        ));
        last.push((
            PAGE_HEIGHT - (top + 50.0),
            vec![(
                36.0,
                "Unless the constituent notifies the bank of errors within 30 days".into(),
            )],
        ));
        // Build one PDF page per statement page by merging into a single content stream is not
        // possible here, so pages are laid out on one tall logical page each and read in order.
        let mut all = Vec::new();
        for (number, page) in pages.iter().enumerate() {
            let mut page = page.clone();
            page.push((
                PAGE_HEIGHT - 812.0,
                vec![(531.5, format!("page {}", number + 1))],
            ));
            all.push(page);
        }
        // `pdf` writes a single page, so a multi-page statement is read page by page below.
        pdf_multi(&all)
    }

    /// Several single-page PDFs cannot be merged by the helper, so build one document with
    /// every page by calling the helper per page and re-using its page objects.
    fn pdf_multi(pages: &[PageLines]) -> Document {
        let mut documents: Vec<Document> = pages
            .iter()
            .map(|lines| {
                let borrowed: Vec<(f64, Vec<(f64, &str)>)> = lines
                    .iter()
                    .map(|(y, cells)| {
                        (
                            *y,
                            cells.iter().map(|(x, text)| (*x, text.as_str())).collect(),
                        )
                    })
                    .collect();
                pdf(&borrowed)
            })
            .collect();
        let mut first = documents.remove(0);
        if documents.is_empty() {
            return first;
        }
        let pages_id = first
            .catalog()
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        for document in documents {
            let extra = merge_page(&mut first, document);
            let pages_dict = first
                .get_object_mut(pages_id)
                .unwrap()
                .as_dict_mut()
                .unwrap();
            let kids = pages_dict.get_mut(b"Kids").unwrap().as_array_mut().unwrap();
            kids.push(extra.into());
            let count = kids.len() as i64;
            pages_dict.set("Count", count);
        }
        first
    }

    fn merge_page(into: &mut Document, mut from: Document) -> lopdf::ObjectId {
        use lopdf::Object;
        let offset = into.max_id;
        from.renumber_objects_with(offset + 1);
        let page_id = *from.get_pages().values().next().unwrap();
        let pages_id = from
            .catalog()
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        let into_pages_id = into
            .catalog()
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        for (id, object) in from.objects {
            if id == pages_id {
                continue;
            }
            if let Object::Dictionary(dict) = &object {
                if dict.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"Catalog") {
                    continue;
                }
            }
            into.objects.insert(id, object);
        }
        if let Ok(dict) = into.get_object_mut(page_id).and_then(Object::as_dict_mut) {
            dict.set("Parent", into_pages_id);
        }
        into.max_id = into
            .objects
            .keys()
            .map(|id| id.0)
            .max()
            .unwrap_or(into.max_id);
        page_id
    }

    const SWIGGY: &str = "SWIGGYBANGALO";

    fn rows() -> Vec<Row> {
        vec![
            Row {
                date: "01-11-2026",
                deposit: false,
                paise: 124_850,
                rrn: "412345678901",
                merchant: SWIGGY,
            },
            Row {
                date: "02-11-2026",
                deposit: true,
                paise: 8_500_000,
                rrn: "412345678902",
                merchant: "SALARYNOVEMBER",
            },
            Row {
                date: "03-11-2026",
                deposit: false,
                paise: 64_000,
                rrn: "412345678903",
                merchant: "NETFLIXCOM",
            },
        ]
    }

    fn parse(document: Document) -> PdfStatement {
        parse_statement(&extract_pages(&bytes(document), None).unwrap()).unwrap()
    }

    #[test]
    fn reads_columns_by_alignment_across_pages_with_wrapped_narration() {
        let statement = parse(statement_pdf(10_000_000, &rows(), None, false));
        assert_eq!(statement.opening_paise, Some(10_000_000));
        assert_eq!(
            statement.closing_paise,
            Some(10_000_000 - 124_850 + 8_500_000 - 64_000)
        );
        assert!(statement.issues.is_empty(), "{:?}", statement.issues);
        let [first, second, third] = statement.transactions.as_slice() else {
            panic!(
                "expected three transactions, got {:?}",
                statement.transactions
            );
        };
        assert_eq!(
            (
                first.date.as_str(),
                first.withdrawal_paise,
                first.deposit_paise
            ),
            ("2026-11-01", Some(124_850), None)
        );
        assert_eq!(
            (
                second.date.as_str(),
                second.deposit_paise,
                second.withdrawal_paise
            ),
            ("2026-11-02", Some(8_500_000), None)
        );
        assert_eq!(
            (third.date.as_str(), third.withdrawal_paise),
            ("2026-11-03", Some(64_000))
        );
        assert_eq!(first.balance_paise, 10_000_000 - 124_850);
        // The first line ran to the column edge, so the next line continues the same word.
        assert_eq!(first.narration, "UPI/DR/412345678901/SWIGGYBANGALORE/OKAXIS/**1234@ybl/NO REMARKS//UPI9X ABC123/01/11/26 2026 14:05:33");
        assert_eq!(first.reference.as_deref(), Some("412345678901"));
        assert_eq!(second.reference.as_deref(), Some("412345678902"));
    }

    #[test]
    fn parsed_statement_proves_it_matches_the_printed_balances() {
        let statement = parse(statement_pdf(10_000_000, &rows(), None, false));
        let parsed = to_parsed("nov.pdf".into(), "sha".into(), statement);
        assert_eq!(parsed.valid_rows.len(), 3);
        assert!(parsed.invalid_rows.is_empty());
        let check = parsed.statement_check.unwrap();
        assert!(check.totals_match && check.every_row_balances);
        assert_eq!(
            (check.deposits_total_paise, check.withdrawals_total_paise),
            (8_500_000, 188_850)
        );
        assert_eq!(parsed.valid_rows[1].direction, Direction::Credit);
        assert_eq!(parsed.valid_rows[0].balance_after_paise, Some(9_875_150));
        assert_eq!(parsed.source_kind, "pdf");
    }

    #[test]
    fn a_row_whose_balance_does_not_follow_is_reported_and_left_out() {
        let statement = parse(statement_pdf(10_000_000, &rows(), None, true));
        let parsed = to_parsed("bad.pdf".into(), "sha".into(), statement);
        assert!(!parsed.statement_check.as_ref().unwrap().every_row_balances);
        assert!(
            parsed
                .invalid_rows
                .iter()
                .any(|issue| issue.row_number == 2
                    && issue.reason.contains("balance does not follow"))
        );
        assert!(parsed.valid_rows.iter().all(|row| row.row_number != 2));
    }

    #[test]
    fn missing_rows_are_caught_by_the_printed_closing_balance() {
        let statement = parse(statement_pdf(10_000_000, &rows(), Some(1), false));
        let parsed = to_parsed("short.pdf".into(), "sha".into(), statement);
        let check = parsed.statement_check.unwrap();
        assert!(check.every_row_balances && !check.totals_match);
        assert!(parsed
            .invalid_rows
            .iter()
            .any(|issue| issue.reason.contains("closing balance")));
    }

    #[test]
    fn a_pdf_without_the_statement_table_is_not_guessed_at() {
        let document = pdf(&[(800.0, vec![(40.0, "Hello there")])]);
        let result = parse_statement(&extract_pages(&bytes(document), None).unwrap());
        assert!(matches!(result, Err(ParseError::UnsupportedLayout(_))));
    }

    #[test]
    fn amounts_and_dates_parse_exactly() {
        assert_eq!(parse_amount("1,83,751.50"), Some(18_375_150));
        assert_eq!(parse_amount("99.9"), Some(9_990));
        assert_eq!(parse_amount("12"), Some(1_200));
        assert_eq!(parse_amount("1,234.50 Dr"), Some(-123_450));
        assert_eq!(parse_amount("500.00 Cr"), Some(50_000));
        assert_eq!(parse_amount("12.345"), None);
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(parse_date("01-11-2026").as_deref(), Some("2026-11-01"));
        assert_eq!(parse_date("05/03/2026").as_deref(), Some("2026-03-05"));
        assert_eq!(parse_date("07-Nov-2026").as_deref(), Some("2026-11-07"));
        assert_eq!(parse_date("31-02-2026"), None);
        assert_eq!(parse_date("Balance"), None);
    }

    fn write(name: &str, document: Document) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("ledger-{name}-{}.pdf", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes(document)).unwrap();
        path
    }

    fn account(database: &Database) -> String {
        use crate::db::repositories::{AccountDraft, AccountRepository};
        AccountRepository::new(database)
            .create(AccountDraft {
                name: "Daily".into(),
                institution: None,
                account_label: None,
            })
            .unwrap()
            .id
    }

    #[test]
    fn weekly_pdf_uploads_add_only_the_new_transactions() {
        let database = Database::open_in_memory().unwrap();
        let account_id = account(&database);
        let service = PdfImportService::new(&database);
        let all = rows();
        let week1 = write("w1", statement_pdf(10_000_000, &all[..2], None, false));
        let preview = service
            .preview(&account_id, week1.to_str().unwrap(), None)
            .unwrap();
        assert_eq!(
            (
                preview.rows_new,
                preview.rows_already_imported,
                preview.source_kind.as_str()
            ),
            (2, 0, "pdf")
        );
        assert_eq!(
            service
                .commit(&account_id, week1.to_str().unwrap(), None)
                .unwrap()
                .rows_imported,
            2
        );

        // The next statement starts from the balance after the first row and repeats the second.
        let week2 = write("w2", statement_pdf(10_000_000 - 124_850, &all, None, false));
        let preview = service
            .preview(&account_id, week2.to_str().unwrap(), None)
            .unwrap();
        assert_eq!((preview.rows_new, preview.rows_already_imported), (1, 2));
        let committed = service
            .commit(&account_id, week2.to_str().unwrap(), None)
            .unwrap();
        assert_eq!(
            (committed.rows_imported, committed.rows_skipped_duplicate),
            (1, 2)
        );
        let stored: i64 = database
            .with_connection(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(stored, 3);
        let note: String = database
            .with_connection(|c| {
                Ok(c.query_row(
                    "SELECT notes FROM import_batches ORDER BY imported_at LIMIT 1",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(note, "Canara PDF statement");
    }

    #[test]
    fn a_statement_that_does_not_add_up_is_not_committed() {
        let database = Database::open_in_memory().unwrap();
        let account_id = account(&database);
        let path = write("bad", statement_pdf(10_000_000, &rows(), None, true));
        let service = PdfImportService::new(&database);
        assert!(service
            .preview(&account_id, path.to_str().unwrap(), None)
            .unwrap()
            .statement_check
            .is_some_and(|c| !c.every_row_balances));
        assert!(service
            .commit(&account_id, path.to_str().unwrap(), None)
            .is_err());
        let stored: i64 = database
            .with_connection(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(stored, 0);
    }

    #[test]
    fn password_protected_statements_ask_for_the_password() {
        let mut document = statement_pdf(10_000_000, &rows(), None, false);
        let version = EncryptionVersion::V2 {
            document: &document,
            owner_password: "owner",
            user_password: "1234abcd",
            key_length: 128,
            permissions: Permissions::all(),
        };
        let state = EncryptionState::try_from(version).unwrap();
        document.encrypt(&state).unwrap();
        let path = write("locked", document);
        let database = Database::open_in_memory().unwrap();
        let account_id = account(&database);
        let service = PdfImportService::new(&database);
        let text = path.to_str().unwrap();
        assert_eq!(
            service
                .preview(&account_id, text, None)
                .unwrap_err()
                .to_string(),
            "PDF_PASSWORD_REQUIRED"
        );
        assert_eq!(
            service
                .preview(&account_id, text, Some("nope"))
                .unwrap_err()
                .to_string(),
            "PDF_PASSWORD_WRONG"
        );
        assert_eq!(
            service
                .commit(&account_id, text, Some("1234abcd"))
                .unwrap()
                .rows_imported,
            3
        );
    }

    /// Rebuilds a PDF from a *masked* layout report (see `pdf_layout`), keeping every real x/y
    /// position but replacing the content with made-up values, then checks the parser finds the
    /// structure the report shows. Run with LEDGER_MASKED_LAYOUT=/path/to/pdf-layout-masked.txt.
    #[test]
    #[ignore = "needs a masked layout file, run on demand"]
    fn parses_the_structure_of_a_real_masked_layout() {
        let path = std::env::var("LEDGER_MASKED_LAYOUT").expect("set LEDGER_MASKED_LAYOUT");
        let report = std::fs::read_to_string(path).unwrap();
        let mut pages: Vec<PageLines> = Vec::new();
        let mut sample_amount = 0u32;
        for line in report.lines() {
            if line.starts_with("=== page") {
                pages.push(Vec::new());
                continue;
            }
            let Some(rest) = line.strip_prefix("y=") else {
                continue;
            };
            let Some(page) = pages.last_mut() else {
                continue;
            };
            let mut parts = rest.split(" | ");
            let y: f64 = parts.next().unwrap().trim().parse().unwrap();
            let mut cells: Cells = Vec::new();
            for part in parts {
                let (position, text) = part.split_once(" \"").unwrap();
                let text = text.trim_end_matches('"');
                let x: f64 = position
                    .trim_start_matches("x=")
                    .split("..")
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let is_amount =
                    text.chars().all(|c| c == '9' || c == ',' || c == '.') && text.contains('.');
                let replaced = if text == "99-99-9999" {
                    "01-11-2026".to_owned()
                } else if is_amount {
                    sample_amount += 1;
                    format!("{}.00", 100 + sample_amount)
                } else if text.starts_with("Chq: ") {
                    format!("Chq: {}", 100_000_000_000u64 + u64::from(sample_amount))
                } else if let Some(narration) = text.strip_prefix("99-99-9999 ") {
                    // A date followed by narration in one cell: the date sits at x, the text after it.
                    cells.push((x, "01-11-2026".to_owned()));
                    cells.push((106.5, narration.to_owned()));
                    continue;
                } else {
                    text.to_owned()
                };
                cells.push((x, replaced));
            }
            page.push((PAGE_HEIGHT - y, cells));
        }
        let statement = parse(pdf_multi(&pages));
        let deposits = statement
            .transactions
            .iter()
            .filter(|t| t.deposit_paise.is_some())
            .count();
        let withdrawals = statement
            .transactions
            .iter()
            .filter(|t| t.withdrawal_paise.is_some())
            .count();
        println!(
            "transactions {} deposits {deposits} withdrawals {withdrawals}",
            statement.transactions.len()
        );
        println!(
            "opening {:?} closing {:?}",
            statement.opening_paise, statement.closing_paise
        );
        println!("parse issues {}", statement.issues.len());
        println!("first narration: {}", statement.transactions[0].narration);
        assert_eq!(statement.transactions.len(), 42);
        assert_eq!((deposits, withdrawals), (11, 31));
        assert!(statement.opening_paise.is_some() && statement.closing_paise.is_some());
        assert!(statement
            .transactions
            .iter()
            .all(|t| t.narration.starts_with("UPI/")));
    }

    #[test]
    fn only_pdf_files_are_accepted() {
        let database = Database::open_in_memory().unwrap();
        let service = PdfImportService::new(&database);
        assert!(service.preview("a", "/tmp/statement.csv", None).is_err());
    }
}
