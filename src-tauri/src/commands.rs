use tauri::State;

use crate::{
    db::{
        repositories::{
            Account, AccountDraft, AccountRepository, BudgetDraft, BudgetRepository, BudgetStatus,
            Category, CategoryRepository, MerchantRule, MerchantRuleDraft, MerchantRuleUpdate,
            OverviewRepository, OverviewSummary, RuleRepository, SummaryOptions, TransactionEdit,
            TransactionPage, TransactionQuery, TransactionRepository,
        },
        settings::{
            AppSettings, CategoryAdminRepository, CategoryDraft, CategoryRow, RecurringRepository,
            RecurringSuggestion, SettingsRepository,
        },
        Database,
    },
    imports::canara_pdf::PdfImportService,
    imports::csv::{CommitResult, CsvImportService, CsvMapping, CsvPreview},
    imports::history::{ImportBatchSummary, ImportHistoryRepository, RangeSummary},
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

#[tauri::command]
pub fn overview_summary(
    today: String,
    options: Option<SummaryOptions>,
    database: State<'_, Database>,
) -> Result<OverviewSummary, String> {
    OverviewRepository::new(&database)
        .summary_with(&today, options.unwrap_or_default())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_expense_categories(database: State<'_, Database>) -> Result<Vec<Category>, String> {
    CategoryRepository::new(&database)
        .list_expense()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn list_merchant_rules(database: State<'_, Database>) -> Result<Vec<MerchantRule>, String> {
    RuleRepository::new(&database)
        .list()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn create_merchant_rule(
    draft: MerchantRuleDraft,
    database: State<'_, Database>,
) -> Result<MerchantRule, String> {
    RuleRepository::new(&database)
        .create(draft)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn update_merchant_rule(
    id: String,
    update: MerchantRuleUpdate,
    database: State<'_, Database>,
) -> Result<MerchantRule, String> {
    RuleRepository::new(&database)
        .update(&id, update)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn list_budget_status(
    month: String,
    today: String,
    database: State<'_, Database>,
) -> Result<Vec<BudgetStatus>, String> {
    BudgetRepository::new(&database)
        .status(&month, &today)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn upsert_budget(draft: BudgetDraft, database: State<'_, Database>) -> Result<(), String> {
    BudgetRepository::new(&database)
        .upsert(draft)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn export_transactions_csv(
    path: String,
    database: State<'_, Database>,
) -> Result<usize, String> {
    database
        .export_transactions_csv(path)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn backup_database(path: String, database: State<'_, Database>) -> Result<(), String> {
    database.backup_to(path).map_err(|error| error.to_string())
}
#[tauri::command]
pub fn restore_database(path: String, database: State<'_, Database>) -> Result<(), String> {
    database
        .restore_from(path)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn clear_personal_data(database: State<'_, Database>) -> Result<(), String> {
    database
        .clear_personal_data()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_account(
    id: String,
    name: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    AccountRepository::new(&database)
        .rename(&id, name)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn archive_account(id: String, database: State<'_, Database>) -> Result<(), String> {
    AccountRepository::new(&database)
        .archive(&id)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn list_categories(database: State<'_, Database>) -> Result<Vec<CategoryRow>, String> {
    CategoryAdminRepository::new(&database)
        .list_all()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn create_category(
    draft: CategoryDraft,
    database: State<'_, Database>,
) -> Result<CategoryRow, String> {
    CategoryAdminRepository::new(&database)
        .create(draft)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn rename_category(
    id: String,
    name: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    CategoryAdminRepository::new(&database)
        .rename(&id, name)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn archive_category(id: String, database: State<'_, Database>) -> Result<(), String> {
    CategoryAdminRepository::new(&database)
        .archive(&id)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn get_settings(database: State<'_, Database>) -> Result<AppSettings, String> {
    SettingsRepository::new(&database)
        .load()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn set_first_day_of_month(day: u8, database: State<'_, Database>) -> Result<(), String> {
    SettingsRepository::new(&database)
        .set_first_day_of_month(day)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn recurring_suggestions(
    database: State<'_, Database>,
) -> Result<Vec<RecurringSuggestion>, String> {
    RecurringRepository::new(&database)
        .suggestions()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn dismiss_recurring(
    merchant_key: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    RecurringRepository::new(&database)
        .dismiss(&merchant_key)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn backup_database_encrypted(
    path: String,
    passphrase: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    database
        .backup_encrypted_to(path, &passphrase)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn restore_database_encrypted(
    path: String,
    passphrase: String,
    database: State<'_, Database>,
) -> Result<(), String> {
    database
        .restore_encrypted_from(path, &passphrase)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_import_batches(
    database: State<'_, Database>,
) -> Result<Vec<ImportBatchSummary>, String> {
    ImportHistoryRepository::new(&database)
        .list()
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn delete_import_batch(id: String, database: State<'_, Database>) -> Result<usize, String> {
    ImportHistoryRepository::new(&database)
        .delete_batch(&id)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn range_summary(
    account_id: Option<String>,
    from: String,
    to: String,
    database: State<'_, Database>,
) -> Result<RangeSummary, String> {
    ImportHistoryRepository::new(&database)
        .range_summary(account_id.as_deref(), &from, &to)
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn delete_transactions_in_range(
    account_id: Option<String>,
    from: String,
    to: String,
    database: State<'_, Database>,
) -> Result<usize, String> {
    ImportHistoryRepository::new(&database)
        .delete_range(account_id.as_deref(), &from, &to)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_pdf_import(
    account_id: String,
    path: String,
    password: Option<String>,
    database: State<'_, Database>,
) -> Result<CsvPreview, String> {
    PdfImportService::new(&database)
        .preview(&account_id, &path, password.as_deref())
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub fn commit_pdf_import(
    account_id: String,
    path: String,
    password: Option<String>,
    database: State<'_, Database>,
) -> Result<CommitResult, String> {
    PdfImportService::new(&database)
        .commit(&account_id, &path, password.as_deref())
        .map_err(|error| error.to_string())
}
