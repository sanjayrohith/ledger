import { invoke } from "@tauri-apps/api/core";

export type Account = { id: string; name: string; institution: string; accountLabel: string | null };
export type AccountDraft = { name: string; institution?: string; accountLabel?: string };
export type CsvMapping = {
  transactionDate: string; description: string; debit?: string; credit?: string; amount?: string;
  direction?: string; valueDate?: string; balance?: string; reference?: string;
};
export type CsvPreview = {
  originalFilename: string; headers: string[]; mapping: CsvMapping; rowsSeen: number; validRows: number;
  invalidRows: { rowNumber: number; reason: string }[];
  duplicateCandidates: { rowNumber: number; kind: "trusted_reference" | "likely_fingerprint" }[];
  debitTotalPaise: number; creditTotalPaise: number; periodStart: string | null; periodEnd: string | null;
};
export type CommitResult = { batchId: string; rowsSeen: number; rowsImported: number; rowsRejected: number; rowsSkippedDuplicate: number };
export type TransactionItem = { id: string; transactionDate: string; direction: string; amountPaise: number; rawDescription: string; merchantOverride: string | null; categoryId: string | null; transactionKind: string; isReviewed: boolean };
export type TransactionPage = { items: TransactionItem[]; total: number };
export type XlsxPreview = { preview: CsvPreview; worksheets: string[]; selectedWorksheet: string };
export type OverviewSummary = { periodStart:string; periodEnd:string; incomePaise:number; expensePaise:number; netCashPaise:number; previousIncomePaise:number; previousExpensePaise:number; unreviewedCount:number; lastImportedAt:string|null; coverageStart:string|null; coverageEnd:string|null; categories:[string,number][] };
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
export const listTransactions = (query: { search?: string; direction?: string; reviewOnly: boolean; limit: number; offset: number }) => invoke<TransactionPage>("list_transactions", { query });
export const editTransaction = (id: string, edit: { merchantOverride?: string; categoryId?: string; isReviewed: boolean }) => invoke("edit_transaction", { id, edit });
export const getOverviewSummary = (today:string) => invoke<OverviewSummary>("overview_summary", { today });
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
