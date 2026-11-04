use tauri::State;

use crate::{
    db::{
        repositories::{
            Account, AccountDraft, AccountRepository, TransactionEdit, TransactionPage,
            TransactionQuery, TransactionRepository,
        },
        Database,
    },
    imports::csv::{CommitResult, CsvImportService, CsvMapping, CsvPreview},
    imports::xlsx::{XlsxImportService, XlsxPreview},
};

#[tauri::command]
pub fn list_accounts(database: State<'_, Database>) -> Result<Vec<Account>, String> {
    AccountRepository::new(&database)
        .list_active()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_account(
    draft: AccountDraft,
    database: State<'_, Database>,
) -> Result<Account, String> {
    AccountRepository::new(&database)
        .create(draft)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_csv_import(
    account_id: String,
    path: String,
    mapping: Option<CsvMapping>,
    database: State<'_, Database>,
) -> Result<CsvPreview, String> {
    CsvImportService::new(&database)
        .preview(&account_id, &path, mapping)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn commit_csv_import(
    account_id: String,
    path: String,
    mapping: CsvMapping,
    database: State<'_, Database>,
) -> Result<CommitResult, String> {
    CsvImportService::new(&database)
        .commit(&account_id, &path, mapping)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_xlsx_import(
    account_id: String,
    path: String,
    worksheet: Option<String>,
    mapping: Option<CsvMapping>,
    database: State<'_, Database>,
) -> Result<XlsxPreview, String> {
    XlsxImportService::new(&database)
        .preview(&account_id, &path, worksheet, mapping)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn commit_xlsx_import(
    account_id: String,
    path: String,
    worksheet: String,
    mapping: CsvMapping,
    database: State<'_, Database>,
) -> Result<CommitResult, String> {
    XlsxImportService::new(&database)
        .commit(&account_id, &path, worksheet, mapping)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_transactions(
    query: TransactionQuery,
    database: State<'_, Database>,
) -> Result<TransactionPage, String> {
    TransactionRepository::new(&database)
        .page(query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn edit_transaction(
    id: String,
    edit: TransactionEdit,
    database: State<'_, Database>,
) -> Result<(), String> {
    TransactionRepository::new(&database)
        .edit(&id, edit)
        .map_err(|error| error.to_string())
}
