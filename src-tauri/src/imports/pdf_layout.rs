//! Local PDF text extraction with positions, plus a *masked* layout dump.
//!
//! The masked dump replaces every digit with `9` and every letter with `X` or `x`, except a
//! short list of generic statement words (column headings and payment-method names). It exists
//! so the shape of a bank statement can be shared without sharing any personal data.

use std::collections::HashSet;
use std::fmt::Write as _;

use pdf_extract::{
    ColorSpace, Document, LoadOptions, MediaBox, OutputDev, OutputError, Path, Transform,
};

#[derive(Debug)]
pub enum PdfError {
    /// The file is encrypted and no password was supplied.
    NeedsPassword,
    /// The supplied password did not open the file.
    WrongPassword,
    Unreadable(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedsPassword => formatter.write_str("this PDF is password-protected"),
            Self::WrongPassword => formatter.write_str("that password did not open the PDF"),
            Self::Unreadable(message) => write!(formatter, "could not read the PDF: {message}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// One character with its position on the page, measured in points from the top left.
#[derive(Debug, Clone)]
pub struct PdfChar {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub size: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PdfPage {
    pub number: u32,
    pub width: f64,
    pub height: f64,
    pub chars: Vec<PdfChar>,
}

#[derive(Default)]
struct Collector {
    pages: Vec<PdfPage>,
    origin: (f64, f64),
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        page_num: u32,
        media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.origin = (media_box.llx, media_box.ury);
        self.pages.push(PdfPage {
            number: page_num,
            width: media_box.urx - media_box.llx,
            height: media_box.ury - media_box.lly,
            chars: Vec::new(),
        });
        Ok(())
    }
    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        spacing: f64,
        font_size: f64,
        char: &str,
    ) -> Result<(), OutputError> {
        let scale = (trm.m11 * trm.m11 + trm.m12 * trm.m12).sqrt();
        if let Some(page) = self.pages.last_mut() {
            page.chars.push(PdfChar {
                x: trm.m31 - self.origin.0,
                y: self.origin.1 - trm.m32,
                width: (width * font_size + spacing) * scale,
                size: font_size * scale,
                text: char.to_owned(),
            });
        }
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn stroke(
        &mut self,
        _: &Transform,
        _: &ColorSpace,
        _: &[f64],
        _: &Path,
    ) -> Result<(), OutputError> {
        Ok(())
    }
    fn fill(
        &mut self,
        _: &Transform,
        _: &ColorSpace,
        _: &[f64],
        _: &Path,
    ) -> Result<(), OutputError> {
        Ok(())
    }
}

/// Reads every page's characters and positions. Nothing leaves the process.
pub fn extract_pages(bytes: &[u8], password: Option<&str>) -> Result<Vec<PdfPage>, PdfError> {
    let mut document =
        Document::load_mem(bytes).map_err(|error| PdfError::Unreadable(error.to_string()))?;
    if document.is_encrypted() {
        // Some banks protect the file with an empty user password, so try that first.
        document = Document::load_mem_with_options(
            bytes,
            LoadOptions::with_password(password.unwrap_or("")),
        )
        .map_err(|_| {
            if password.is_none() {
                PdfError::NeedsPassword
            } else {
                PdfError::WrongPassword
            }
        })?;
    }
    let mut collector = Collector::default();
    pdf_extract::output_doc(&document, &mut collector)
        .map_err(|error| PdfError::Unreadable(error.to_string()))?;
    Ok(collector.pages)
}

/// A run of text on one line, split from its neighbours by a column-sized gap.
#[derive(Debug, Clone)]
pub struct Cell {
    pub x_start: f64,
    pub x_end: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub y: f64,
    pub cells: Vec<Cell>,
}

/// Characters that share a baseline, ordered left to right.
pub struct CharLine<'a> {
    pub y: f64,
    pub chars: Vec<&'a PdfChar>,
}

/// Groups a page's characters into lines by baseline, ordered from the top of the page.
pub fn char_lines(page: &PdfPage) -> Vec<CharLine<'_>> {
    let mut chars: Vec<&PdfChar> = page.chars.iter().collect();
    chars.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<CharLine<'_>> = Vec::new();
    for ch in chars {
        let tolerance = (ch.size * 0.4).max(1.5);
        match lines.last_mut() {
            Some(line) if (ch.y - line.y).abs() <= tolerance => line.chars.push(ch),
            _ => lines.push(CharLine {
                y: ch.y,
                chars: vec![ch],
            }),
        }
    }
    for line in &mut lines {
        line.chars.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    lines
}

/// Joins characters left to right, adding a space where there is a visible gap.
pub fn join_chars(chars: &[&PdfChar]) -> String {
    let mut text = String::new();
    let mut end = f64::NEG_INFINITY;
    for ch in chars {
        if !text.is_empty() && ch.x - end > ch.size * 0.15 && !text.ends_with(' ') && ch.text != " "
        {
            text.push(' ');
        }
        text.push_str(&ch.text);
        end = end.max(ch.x + ch.width);
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Groups characters into lines (by baseline) and cells (by horizontal gaps).
pub fn group_lines(page: &PdfPage) -> Vec<Line> {
    char_lines(page)
        .into_iter()
        .map(|line| {
            let mut cells: Vec<(f64, f64, Vec<&PdfChar>)> = Vec::new();
            for ch in line.chars {
                let end = ch.x + ch.width;
                let gap_limit = (ch.size * 1.6).max(6.0);
                match cells.last_mut() {
                    Some((_, cell_end, members)) if ch.x - *cell_end <= gap_limit => {
                        members.push(ch);
                        *cell_end = cell_end.max(end);
                    }
                    _ => cells.push((ch.x, end, vec![ch])),
                }
            }
            let cells = cells
                .into_iter()
                .map(|(x_start, x_end, members)| Cell {
                    x_start,
                    x_end,
                    text: join_chars(&members),
                })
                .filter(|cell| !cell.text.is_empty())
                .collect::<Vec<_>>();
            Line { y: line.y, cells }
        })
        .filter(|line| !line.cells.is_empty())
        .collect()
}

/// Generic statement vocabulary that is kept readable in the masked dump. Nothing here can
/// identify a person: column headings, payment-method names and common narration words.
const SAFE_WORDS: &[&str] = &[
    "date",
    "dt",
    "txn",
    "tran",
    "trans",
    "transaction",
    "particulars",
    "particular",
    "description",
    "narration",
    "details",
    "remarks",
    "chq",
    "cheque",
    "ref",
    "reference",
    "no",
    "number",
    "value",
    "withdrawal",
    "withdrawals",
    "withdraw",
    "deposit",
    "deposits",
    "debit",
    "debits",
    "credit",
    "credits",
    "dr",
    "cr",
    "balance",
    "bal",
    "amount",
    "amt",
    "opening",
    "closing",
    "brought",
    "carried",
    "forward",
    "fwd",
    "total",
    "totals",
    "page",
    "of",
    "statement",
    "account",
    "from",
    "to",
    "period",
    "sl",
    "srl",
    "id",
    "utr",
    "upi",
    "neft",
    "imps",
    "rtgs",
    "ach",
    "atm",
    "pos",
    "ecom",
    "nach",
    "int",
    "interest",
    "charges",
    "charge",
    "by",
    "transfer",
    "cash",
    "clg",
    "clearing",
    "inr",
    "rs",
    "and",
    "for",
    "the",
    "on",
    "at",
    "sms",
    "pay",
    "payment",
    "bank",
    "branch",
    "ifsc",
    "micr",
    "type",
    "mode",
];

fn safe_words() -> HashSet<&'static str> {
    SAFE_WORDS.iter().copied().collect()
}

const KEPT_PUNCTUATION: &str = " ,.-/:()₹*&#@_+%";

/// Masks one cell: digits become `9`, letters `X`/`x`, safe words stay readable.
pub fn mask_text(text: &str) -> String {
    let safe = safe_words();
    let mut output = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut String| {
        if word.is_empty() {
            return;
        }
        if safe.contains(word.to_lowercase().as_str()) {
            output.push_str(word);
        } else {
            output.extend(
                word.chars()
                    .map(|c| if c.is_uppercase() { 'X' } else { 'x' }),
            );
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            flush(&mut word, &mut output);
            if c.is_numeric() {
                output.push('9');
            } else if KEPT_PUNCTUATION.contains(c) {
                output.push(c);
            } else {
                output.push('?');
            }
        }
    }
    flush(&mut word, &mut output);
    output
}

/// Confirms a masked string holds nothing beyond masks, `9`s, safe words and punctuation.
pub fn verify_masked(masked: &str) -> Result<(), String> {
    let safe = safe_words();
    let mut word = String::new();
    let check_word = |word: &str| -> Result<(), String> {
        if word.is_empty()
            || word.chars().all(|c| c == 'X' || c == 'x')
            || safe.contains(word.to_lowercase().as_str())
        {
            Ok(())
        } else {
            Err(format!("unmasked word found: {word}"))
        }
    };
    for c in masked.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            check_word(&word)?;
            word.clear();
            if c.is_numeric() && c != '9' {
                return Err(format!("unmasked digit found: {c}"));
            }
            if !(c.is_numeric() || KEPT_PUNCTUATION.contains(c) || c == '?') {
                return Err(format!("unexpected character: {c:?}"));
            }
        }
    }
    check_word(&word)
}

fn round_half(value: f64) -> f64 {
    (value * 2.0).round() / 2.0
}

/// Builds the shareable layout report. Every cell is masked and verified before it is written.
pub fn masked_layout(pages: &[PdfPage], was_encrypted: bool) -> Result<String, String> {
    let mut report = String::new();
    writeln!(report, "LEDGER MASKED PDF LAYOUT").unwrap();
    writeln!(report, "pages: {}", pages.len()).unwrap();
    writeln!(
        report,
        "password protected: {}",
        if was_encrypted { "yes" } else { "no" }
    )
    .unwrap();
    writeln!(
        report,
        "Every letter is X or x and every digit is 9, except common statement words."
    )
    .unwrap();
    for page in pages {
        writeln!(
            report,
            "\n=== page {} ({:.0} x {:.0} pt) ===",
            page.number, page.width, page.height
        )
        .unwrap();
        for line in group_lines(page) {
            let mut row = format!("y={:>6.1}", round_half(line.y));
            for cell in &line.cells {
                let masked = mask_text(&cell.text);
                verify_masked(&masked)?;
                write!(
                    row,
                    " | x={:.1}..{:.1} \"{masked}\"",
                    round_half(cell.x_start),
                    round_half(cell.x_end)
                )
                .unwrap();
            }
            writeln!(report, "{row}").unwrap();
        }
    }
    Ok(report)
}

#[cfg(test)]
pub(crate) mod test_support {
    use lopdf::{
        content::{Content, Operation},
        dictionary, Document, Object, Stream,
    };

    /// Builds a one-page PDF with text at exact positions (`y` measured from the page bottom).
    pub fn pdf(lines: &[(f64, Vec<(f64, &str)>)]) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        let resources_id =
            doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
        let mut operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 9.into()]),
        ];
        let mut last = (0.0, 0.0);
        for (y, cells) in lines {
            for (x, text) in cells {
                operations.push(Operation::new(
                    "Td",
                    vec![(*x - last.0).into(), (*y - last.1).into()],
                ));
                operations.push(Operation::new("Tj", vec![Object::string_literal(*text)]));
                last = (*x, *y);
            }
        }
        operations.push(Operation::new("ET", vec![]));
        let content = Content { operations };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content_id, "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal("ledger-test-id-0001"),
                Object::string_literal("ledger-test-id-0001"),
            ]),
        );
        doc
    }

    pub fn bytes(mut doc: Document) -> Vec<u8> {
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// Rough width of `text` in 9pt Helvetica, for right-aligning numbers in fixtures.
    pub fn width_9pt(text: &str) -> f64 {
        text.chars()
            .map(|c| if c.is_ascii_digit() { 5.0 } else { 2.5 })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};

    use super::test_support::{bytes, pdf};
    use super::*;

    fn statement() -> Document {
        pdf(&[
            (
                800.0,
                vec![
                    (40.0, "Account Name: PRIYA SHARMA"),
                    (300.0, "Account No: 1234567890123"),
                ],
            ),
            (
                760.0,
                vec![
                    (40.0, "Date"),
                    (110.0, "Particulars"),
                    (360.0, "Withdrawals"),
                    (440.0, "Deposits"),
                    (510.0, "Balance"),
                ],
            ),
            (
                740.0,
                vec![
                    (40.0, "01-11-2026"),
                    (110.0, "UPI/DR/412345678901/Swiggy Bangalore"),
                    (365.0, "1,248.50"),
                    (500.0, "98,751.50"),
                ],
            ),
            (
                728.0,
                vec![
                    (40.0, "02-11-2026"),
                    (110.0, "SALARY NOVEMBER ACME PVT LTD"),
                    (440.0, "85,000.00"),
                    (500.0, "1,83,751.50"),
                ],
            ),
        ])
    }

    #[test]
    fn extracts_positions_and_groups_lines_into_columns() {
        let pages = extract_pages(&bytes(statement()), None).unwrap();
        assert_eq!(pages.len(), 1);
        assert!((pages[0].height - 842.0).abs() < 0.1);
        let lines = group_lines(&pages[0]);
        let swiggy = lines
            .iter()
            .find(|l| l.cells.iter().any(|c| c.text.contains("Swiggy")))
            .unwrap();
        let texts: Vec<_> = swiggy.cells.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "01-11-2026",
                "UPI/DR/412345678901/Swiggy Bangalore",
                "1,248.50",
                "98,751.50"
            ]
        );
        assert!(swiggy.cells[2].x_start > swiggy.cells[1].x_end);
        // Lines are ordered from the top of the page down.
        assert!(lines[0].y < lines[1].y && lines[1].y < lines[2].y);
    }

    #[test]
    fn masked_layout_contains_no_names_numbers_or_merchants() {
        let pages = extract_pages(&bytes(statement()), None).unwrap();
        let report = masked_layout(&pages, false).unwrap();
        for secret in [
            "PRIYA",
            "SHARMA",
            "1234567890123",
            "Swiggy",
            "Bangalore",
            "ACME",
            "1,248",
            "85,000",
            "412345678901",
            "SALARY",
        ] {
            assert!(!report.contains(secret), "leaked {secret}\n{report}");
        }
        // The structure a parser needs survives.
        assert!(
            report.contains("Date")
                && report.contains("Particulars")
                && report.contains("Withdrawals")
        );
        assert!(report.contains("99-99-9999"));
        assert!(report.contains("UPI/DR/999999999999/Xxxxxx Xxxxxxxxx"));
        assert!(report.contains("9,99,999.99"));
        assert!(report.contains("Account Xxxx: XXXXX XXXXXX"));
        verify_masked(&mask_text("PRIYA SHARMA priya@okbank 98 765")).unwrap();
    }

    #[test]
    fn verify_rejects_unmasked_content() {
        assert!(verify_masked("Swiggy").is_err());
        assert!(verify_masked("12.50").is_err());
        assert!(verify_masked("Date 99-99-9999").is_ok());
        assert_eq!(mask_text("a\u{0001}b"), "x?x");
    }

    #[test]
    fn password_protected_files_need_the_right_password() {
        let mut document = statement();
        let version = EncryptionVersion::V2 {
            document: &document,
            owner_password: "owner",
            user_password: "secret123",
            key_length: 128,
            permissions: Permissions::all(),
        };
        let state = EncryptionState::try_from(version).unwrap();
        document.encrypt(&state).unwrap();
        let data = bytes(document);

        assert!(matches!(
            extract_pages(&data, None),
            Err(PdfError::NeedsPassword)
        ));
        assert!(matches!(
            extract_pages(&data, Some("wrong")),
            Err(PdfError::WrongPassword)
        ));
        let pages = extract_pages(&data, Some("secret123")).unwrap();
        assert!(pages[0]
            .chars
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>()
            .contains("Swiggy"));
    }

    /// Writes the synthetic statement to `LEDGER_SAMPLE_PDF` for trying the example by hand.
    #[test]
    #[ignore = "writes a sample file, run on demand"]
    fn write_sample_pdf() {
        let path = std::env::var("LEDGER_SAMPLE_PDF").expect("set LEDGER_SAMPLE_PDF");
        std::fs::write(path, bytes(statement())).unwrap();
    }

    #[test]
    fn garbage_is_reported_as_unreadable() {
        assert!(matches!(
            extract_pages(b"not a pdf", None),
            Err(PdfError::Unreadable(_))
        ));
    }
}
