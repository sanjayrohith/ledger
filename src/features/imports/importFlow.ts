import type { CsvPreview } from "../../app/commands";

export function canCommitImport(accountId: string, path: string, preview: CsvPreview | null): boolean {
  return Boolean(accountId && path && preview && preview.rowsNew > 0);
}

export function selectedCsvPath(selection: string | string[] | null): string | null {
  return typeof selection === "string" ? selection : null;
}
