use std::fmt;

use time::{format_description, Date, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Expense,
    Income,
    Transfer,
    Refund,
    Reversal,
    Fee,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalTransaction {
    pub transaction_date: Date,
    pub value_date: Option<Date>,
    pub raw_description: String,
    pub amount_paise: i64,
    pub direction: Direction,
    pub balance_after_paise: Option<i64>,
    pub reference_raw: Option<String>,
    pub transaction_kind: TransactionKind,
}

impl CanonicalTransaction {
    pub fn new(
        transaction_date: Date,
        raw_description: String,
        amount_paise: i64,
        direction: Direction,
    ) -> Result<Self, MoneyParseError> {
        if amount_paise <= 0 {
            return Err(MoneyParseError::ZeroOrNegative);
        }

        Ok(Self {
            transaction_date,
            value_date: None,
            raw_description,
            amount_paise,
            direction,
            balance_after_paise: None,
            reference_raw: None,
            transaction_kind: TransactionKind::Unknown,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoneyParseError {
    Empty,
    Invalid,
    TooManyFractionalDigits,
    Overflow,
    ZeroOrNegative,
}

impl fmt::Display for MoneyParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "amount is empty",
            Self::Invalid => "amount must use digits with an optional two-digit decimal fraction",
            Self::TooManyFractionalDigits => "amount has more than two fractional digits",
            Self::Overflow => "amount is outside Ledger's supported range",
            Self::ZeroOrNegative => "amount must be greater than zero",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for MoneyParseError {}

pub fn parse_positive_paise(input: &str) -> Result<i64, MoneyParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(MoneyParseError::Empty);
    }

    let without_currency = trimmed.strip_prefix('₹').unwrap_or(trimmed).trim();
    if without_currency.starts_with('-') || without_currency.starts_with('+') {
        return Err(MoneyParseError::ZeroOrNegative);
    }

    let mut parts = without_currency.split('.');
    let whole = parts.next().ok_or(MoneyParseError::Invalid)?;
    let fraction = parts.next();
    if parts.next().is_some() {
        return Err(MoneyParseError::Invalid);
    }

    let whole_digits: String = whole
        .chars()
        .filter(|character| *character != ',')
        .collect();
    if whole_digits.is_empty()
        || !whole_digits
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(MoneyParseError::Invalid);
    }

    let fraction_paise = match fraction {
        None | Some("") => 0,
        Some(value) if value.len() > 2 => return Err(MoneyParseError::TooManyFractionalDigits),
        Some(value) if !value.chars().all(|character| character.is_ascii_digit()) => {
            return Err(MoneyParseError::Invalid)
        }
        Some(value) if value.len() == 1 => {
            value
                .parse::<i64>()
                .map_err(|_| MoneyParseError::Overflow)?
                * 10
        }
        Some(value) => value
            .parse::<i64>()
            .map_err(|_| MoneyParseError::Overflow)?,
    };

    let whole_paise = whole_digits
        .parse::<i64>()
        .map_err(|_| MoneyParseError::Overflow)?
        .checked_mul(100)
        .ok_or(MoneyParseError::Overflow)?;
    let paise = whole_paise
        .checked_add(fraction_paise)
        .ok_or(MoneyParseError::Overflow)?;

    if paise == 0 {
        return Err(MoneyParseError::ZeroOrNegative);
    }
    Ok(paise)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateParseError {
    Invalid,
}

impl fmt::Display for DateParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("date must use ISO calendar format YYYY-MM-DD")
    }
}

impl std::error::Error for DateParseError {}

pub fn parse_iso_date(input: &str) -> Result<Date, DateParseError> {
    if input != input.trim() || input.len() != 10 {
        return Err(DateParseError::Invalid);
    }
    let format = format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|_| DateParseError::Invalid)?;
    Date::parse(input, &format).map_err(|_| DateParseError::Invalid)
}

pub fn utc_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamps use a valid RFC 3339 format")
}

#[cfg(test)]
mod tests {
    use super::{
        parse_iso_date, parse_positive_paise, CanonicalTransaction, Direction, MoneyParseError,
    };

    #[test]
    fn parses_inr_amounts_without_floating_point() {
        assert_eq!(parse_positive_paise("₹1,234.56"), Ok(123_456));
        assert_eq!(parse_positive_paise("0.5"), Ok(50));
        assert_eq!(parse_positive_paise("12"), Ok(1_200));
    }

    #[test]
    fn rejects_zero_negative_and_overprecise_amounts() {
        assert_eq!(
            parse_positive_paise("0"),
            Err(MoneyParseError::ZeroOrNegative)
        );
        assert_eq!(
            parse_positive_paise("-12.00"),
            Err(MoneyParseError::ZeroOrNegative)
        );
        assert_eq!(
            parse_positive_paise("12.345"),
            Err(MoneyParseError::TooManyFractionalDigits)
        );
    }

    #[test]
    fn accepts_only_real_iso_calendar_dates() {
        assert!(parse_iso_date("2026-02-28").is_ok());
        assert!(parse_iso_date("2026-02-30").is_err());
        assert!(parse_iso_date("28-02-2026").is_err());
    }

    #[test]
    fn canonical_rows_require_positive_paise() {
        let date = parse_iso_date("2026-01-02").expect("valid fixture date");
        assert!(CanonicalTransaction::new(
            date,
            "Synthetic merchant".into(),
            501,
            Direction::Debit
        )
        .is_ok());
        assert!(
            CanonicalTransaction::new(date, "Synthetic merchant".into(), 0, Direction::Debit)
                .is_err()
        );
    }
}
