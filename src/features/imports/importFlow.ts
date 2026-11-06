import type { CsvPreview } from "../../app/commands";

export function canCommitImport(accountId: string, path: string, preview: CsvPreview | null): boolean {
  if (!accountId || !path || !preview || preview.rowsNew <= 0) return false;
  const check = preview.statementCheck;
  if (!check) return true;
  // A PDF is only importable when its rows agree with the balances printed on the statement.
  const hasBoth = check.openingBalancePaise !== null && check.closingBalancePaise !== null;
  return check.everyRowBalances && (!hasBoth || check.totalsMatch);
}

export function statementKind(path: string): "pdf" | "xlsx" | "csv" {
  const lower = path.toLowerCase();
  return lower.endsWith(".pdf") ? "pdf" : lower.endsWith(".xlsx") ? "xlsx" : "csv";
}

export function selectedCsvPath(selection: string | string[] | null): string | null {
  return typeof selection === "string" ? selection : null;
}
