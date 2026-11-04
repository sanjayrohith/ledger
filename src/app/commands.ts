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
