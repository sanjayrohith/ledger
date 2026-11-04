pub const APP_NAME: &str = "Ledger";

pub mod commands;
pub mod db;
pub mod domain;
pub mod imports;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let database = db::Database::open_in_app_data(app.handle())
                .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_accounts,
            commands::create_account,
            commands::preview_csv_import,
            commands::commit_csv_import,
            commands::preview_xlsx_import,
            commands::commit_xlsx_import,
            commands::list_transactions,
            commands::edit_transaction,
            commands::overview_summary,
            commands::list_expense_categories,
            commands::list_merchant_rules,
            commands::create_merchant_rule,
            commands::list_budget_status,
            commands::upsert_budget,
        ])
        .run(tauri::generate_context!())
        .expect("Ledger failed to start");
}

#[cfg(test)]
mod tests {
    use super::APP_NAME;

    #[test]
    fn uses_the_ledger_application_identity() {
        assert_eq!(APP_NAME, "Ledger");
    }
}
