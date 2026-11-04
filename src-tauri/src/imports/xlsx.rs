use std::{fs, path::Path};

use calamine::{open_workbook_auto, Reader};
use csv::Writer;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    db::Database,
    imports::csv::{
        parse_csv_bytes, CommitResult, CsvImportService, CsvMapping, CsvPreview, ImportError,
    },
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxPreview {
    pub preview: CsvPreview,
    pub worksheets: Vec<String>,
    pub selected_worksheet: String,
}

pub struct XlsxImportService<'database> {
    database: &'database Database,
}

impl<'database> XlsxImportService<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn preview(
        &self,
        account_id: &str,
        path: &str,
        worksheet: Option<String>,
        mapping: Option<CsvMapping>,
    ) -> Result<XlsxPreview, ImportError> {
        let (parsed, worksheets, selected_worksheet) = parse_xlsx(path, worksheet, mapping)?;
        Ok(XlsxPreview {
            preview: CsvImportService::new(self.database).preview_parsed(account_id, parsed)?,
            worksheets,
            selected_worksheet,
        })
    }

    pub fn commit(
        &self,
        account_id: &str,
        path: &str,
        worksheet: String,
        mapping: CsvMapping,
    ) -> Result<CommitResult, ImportError> {
        let (parsed, _, _) = parse_xlsx(path, Some(worksheet), Some(mapping))?;
        CsvImportService::new(self.database).commit_parsed(account_id, parsed)
    }
}

fn parse_xlsx(
    path: &str,
    worksheet: Option<String>,
    mapping: Option<CsvMapping>,
) -> Result<(super::csv::ParsedCsv, Vec<String>, String), ImportError> {
    let path = Path::new(path);
    if path.extension().and_then(|extension| extension.to_str()) != Some("xlsx") {
        return Err(ImportError::InvalidPath(
            "select a .xlsx statement file".to_owned(),
        ));
    }
    let original_bytes = fs::read(path)?;
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| ImportError::InvalidPath(format!("could not read XLSX: {error}")))?;
    let worksheets = workbook.sheet_names().to_vec();
    let selected_worksheet = worksheet
        .filter(|name| worksheets.contains(name))
        .or_else(|| worksheets.first().cloned())
        .ok_or_else(|| ImportError::InvalidPath("XLSX has no worksheets".to_owned()))?;
    let range = workbook
        .worksheet_range(&selected_worksheet)
        .map_err(|error| ImportError::InvalidPath(format!("could not read worksheet: {error}")))?;
    if range.height() < 2 {
        return Err(ImportError::InvalidPath(
            "worksheet has no header and data rows".to_owned(),
        ));
    }

    let mut csv = Writer::from_writer(Vec::new());
    for row in range.rows() {
        let values = row.iter().map(ToString::to_string).collect::<Vec<_>>();
        csv.write_record(values).map_err(ImportError::Csv)?;
    }
    let csv_bytes = csv
        .into_inner()
        .map_err(|error| ImportError::Io(error.into_error()))?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ImportError::InvalidPath("selected file has no valid filename".to_owned()))?
        .to_owned();
    let sha256 = Sha256::digest(&original_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let parsed = parse_csv_bytes(&csv_bytes, filename, sha256, "canara_xlsx", mapping)?;
    Ok((parsed, worksheets, selected_worksheet))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rust_xlsxwriter::Workbook;
    use uuid::Uuid;

    use super::XlsxImportService;
    use crate::db::{
        repositories::{AccountDraft, AccountRepository},
        Database,
    };

    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!("ledger-synthetic-{}.xlsx", Uuid::new_v4()));
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("Statement").expect("sheet name");
        for (column, header) in [
            "Transaction Date",
            "Narration",
            "Withdrawal",
            "Deposit",
            "UTR",
        ]
        .iter()
        .enumerate()
        {
            sheet
                .write_string(0, column as u16, *header)
                .expect("header");
        }
        for (column, value) in ["2026-01-10", "SYNTHETIC XLSX", "125.50", "", "SYN-XLSX-1"]
            .iter()
            .enumerate()
        {
            sheet.write_string(1, column as u16, *value).expect("row");
        }
        workbook.save(&path).expect("synthetic xlsx");
        path
    }

    #[test]
    fn previews_and_commits_a_synthetic_xlsx_worksheet() {
        let database = Database::open_in_memory().expect("database");
        let account_id = AccountRepository::new(&database)
            .create(AccountDraft {
                name: "Synthetic XLSX".into(),
                institution: None,
                account_label: None,
            })
            .expect("account")
            .id;
        let path = fixture();
        let service = XlsxImportService::new(&database);
        let preview = service
            .preview(&account_id, path.to_str().expect("path"), None, None)
            .expect("preview");
        assert_eq!(preview.worksheets, ["Statement"]);
        assert_eq!(preview.preview.valid_rows, 1);
        let result = service
            .commit(
                &account_id,
                path.to_str().expect("path"),
                preview.selected_worksheet,
                preview.preview.mapping,
            )
            .expect("commit");
        assert_eq!(result.rows_imported, 1);
        fs::remove_file(path).expect("cleanup");
    }
}
