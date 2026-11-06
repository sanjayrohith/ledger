import { invoke } from "@tauri-apps/api/core";

export type Account = { id: string; name: string; institution: string; accountLabel: string | null };
export type AppSettings = { firstDayOfMonth: number; currency: string; databasePath: string | null };
export type CategoryRow = { id: string; name: string; kind: string; isSystem: boolean };
export type RecurringSuggestion = { merchantKey: string; merchant: string; typicalAmountPaise: number; monthsSeen: number; lastSeen: string };
export type SummaryOptions = { month?: string; rangeMonths?: number };
export type TransactionFilters = { search?: string; direction?: string; accountId?: string; categoryId?: string; dateFrom?: string; dateTo?: string };
export type AccountDraft = { name: string; institution?: string; accountLabel?: string };
export type CsvMapping = {
  transactionDate: string; description: string; debit?: string; credit?: string; amount?: string;
  direction?: string; valueDate?: string; balance?: string; reference?: string;
};
export type StatementCheck = { openingBalancePaise: number | null; closingBalancePaise: number | null; depositsTotalPaise: number; withdrawalsTotalPaise: number; totalsMatch: boolean; everyRowBalances: boolean };
export type CsvPreview = {
  originalFilename: string; headers: string[]; mapping: CsvMapping; rowsSeen: number; validRows: number;
  invalidRows: { rowNumber: number; reason: string }[];
  duplicateCandidates: { rowNumber: number; kind: "trusted_reference" | "likely_fingerprint" }[];
  rowsNew: number; rowsAlreadyImported: number; newDebitTotalPaise: number; newCreditTotalPaise: number;
  sourceKind: "csv" | "xlsx" | "pdf"; statementCheck: StatementCheck | null;
  debitTotalPaise: number; creditTotalPaise: number; periodStart: string | null; periodEnd: string | null;
};
export type CommitResult = { batchId: string; rowsSeen: number; rowsImported: number; rowsRejected: number; rowsSkippedDuplicate: number };
export type TransactionItem = { id: string; transactionDate: string; direction: string; amountPaise: number; rawDescription: string; merchantOverride: string | null; categoryId: string | null; transactionKind: string; isReviewed: boolean; accountName: string; categoryName: string | null; sourceFilename: string | null; importedAt: string | null; sourceRowNumber: number | null };
export type TransactionPage = { items: TransactionItem[]; total: number };
export type XlsxPreview = { preview: CsvPreview; worksheets: string[]; selectedWorksheet: string };
export type OverviewSummary = { periodLabel:string; rangeMonths:number; firstDayOfMonth:number; periodStart:string; periodEnd:string; incomePaise:number; expensePaise:number; netCashPaise:number; previousIncomePaise:number; previousExpensePaise:number; unreviewedCount:number; lastImportedAt:string|null; coverageStart:string|null; coverageEnd:string|null; categories:[string,number][]; trend:[string,number][]; merchants:[string,number][]; budgetAlerts:[string,number,number][] };
export type Category = { id:string; name:string; kind:string };
export type MerchantRule = { id:string; name:string; matchType:string; pattern:string; merchantName:string|null; categoryId:string|null; categoryName:string|null; transactionKind:string|null; priority:number; isEnabled:boolean; explanation:string };
export type BudgetStatus = { categoryId:string; categoryName:string; month:string; amountPaise:number; spentPaise:number; remainingPaise:number; remainingDays:number; remainingPerDayPaise:number };

export const listAccounts = () => invoke<Account[]>("list_accounts");
export const createAccount = (draft: AccountDraft) => invoke<Account>("create_account", { draft });
export const previewCsvImport = (accountId: string, path: string, mapping?: CsvMapping) =>
  invoke<CsvPreview>("preview_csv_import", { accountId, path, mapping });
export const commitCsvImport = (accountId: string, path: string, mapping: CsvMapping) =>
  invoke<CommitResult>("commit_csv_import", { accountId, path, mapping });
export const previewXlsxImport = (accountId: string, path: string, worksheet?: string, mapping?: CsvMapping) => invoke<XlsxPreview>("preview_xlsx_import", { accountId, path, worksheet, mapping });
export const commitXlsxImport = (accountId: string, path: string, worksheet: string, mapping: CsvMapping) => invoke<CommitResult>("commit_xlsx_import", { accountId, path, worksheet, mapping });
export const listTransactions = (query: TransactionFilters & { reviewOnly: boolean; limit: number; offset: number }) => invoke<TransactionPage>("list_transactions", { query });
export const editTransaction = (id: string, edit: { merchantOverride?: string; categoryId?: string; isReviewed: boolean }) => invoke("edit_transaction", { id, edit });
export const getOverviewSummary = (today:string, options?:SummaryOptions) => invoke<OverviewSummary>("overview_summary", { today, options });
export const listExpenseCategories = () => invoke<Category[]>("list_expense_categories");
export const listMerchantRules = () => invoke<MerchantRule[]>("list_merchant_rules");
export const createMerchantRule = (draft: { name:string; matchType:string; pattern:string; merchantName?:string; categoryId?:string; transactionKind?:string; priority:number }) => invoke<MerchantRule>("create_merchant_rule", { draft });
export const updateMerchantRule = (id:string, update: { draft:{ name:string; matchType:string; pattern:string; merchantName?:string; categoryId?:string; transactionKind?:string; priority:number }; isEnabled:boolean }) => invoke<MerchantRule>("update_merchant_rule", { id, update });
export const exportTransactionsCsv = (path:string) => invoke<number>("export_transactions_csv", { path });
export const backupDatabase = (path:string) => invoke("backup_database", { path });
export const restoreDatabase = (path:string) => invoke("restore_database", { path });
export const clearPersonalData = () => invoke("clear_personal_data");
export const listBudgetStatus = (month:string, today:string) => invoke<BudgetStatus[]>("list_budget_status", { month, today });
export const upsertBudget = (draft:{categoryId:string; month:string; amountPaise:number}) => invoke("upsert_budget", { draft });

export const renameAccount = (id:string, name:string) => invoke("rename_account", { id, name });
export const archiveAccount = (id:string) => invoke("archive_account", { id });
export const listCategories = () => invoke<CategoryRow[]>("list_categories");
export const createCategory = (draft:{ name:string; kind:string }) => invoke<CategoryRow>("create_category", { draft });
export const renameCategory = (id:string, name:string) => invoke("rename_category", { id, name });
export const archiveCategory = (id:string) => invoke("archive_category", { id });
export const getSettings = () => invoke<AppSettings>("get_settings");
export const setFirstDayOfMonth = (day:number) => invoke("set_first_day_of_month", { day });
export const recurringSuggestions = () => invoke<RecurringSuggestion[]>("recurring_suggestions");
export const dismissRecurring = (merchantKey:string) => invoke("dismiss_recurring", { merchantKey });
export const backupDatabaseEncrypted = (path:string, passphrase:string) => invoke("backup_database_encrypted", { path, passphrase });
export const restoreDatabaseEncrypted = (path:string, passphrase:string) => invoke("restore_database_encrypted", { path, passphrase });

export type ImportBatchSummary = { id: string; accountId: string; accountName: string; filename: string; importedAt: string; periodStart: string | null; periodEnd: string | null; rowsSeen: number; rowsImported: number; rowsSkippedDuplicate: number; rowsRejected: number; transactionsRemaining: number; editedByYou: number };
export type RangeSummary = { count: number; editedByYou: number; debitTotalPaise: number; creditTotalPaise: number };
export const listImportBatches = () => invoke<ImportBatchSummary[]>("list_import_batches");
export const deleteImportBatch = (id:string) => invoke<number>("delete_import_batch", { id });
export const rangeSummary = (accountId:string|undefined, from:string, to:string) => invoke<RangeSummary>("range_summary", { accountId, from, to });
export const deleteTransactionsInRange = (accountId:string|undefined, from:string, to:string) => invoke<number>("delete_transactions_in_range", { accountId, from, to });
export const previewPdfImport = (accountId:string, path:string, password?:string) => invoke<CsvPreview>("preview_pdf_import", { accountId, path, password });
export const commitPdfImport = (accountId:string, path:string, password?:string) => invoke<CommitResult>("commit_pdf_import", { accountId, path, password });
