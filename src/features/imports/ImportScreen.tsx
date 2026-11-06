import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import type { Account, CommitResult, CsvMapping, CsvPreview, XlsxPreview } from "../../app/commands";
import { commitCsvImport, commitXlsxImport, createAccount, listAccounts, previewCsvImport, previewXlsxImport } from "../../app/commands";
import { ImportHistoryPanels } from "./ImportHistoryPanels";
import { canCommitImport, selectedCsvPath } from "./importFlow";

const money = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" });

function paise(value: number) {
  return money.format(value / 100);
}

type Props = { onError: (message: string) => void };

export function ImportScreen({ onError }: Props) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountId, setAccountId] = useState("");
  const [newAccountName, setNewAccountName] = useState("");
  const [path, setPath] = useState("");
  const [preview, setPreview] = useState<CsvPreview | null>(null);
  const [result, setResult] = useState<CommitResult | null>(null);
  const [worksheet, setWorksheet] = useState("");
  const [xlsx, setXlsx] = useState(false);
  const [worksheets, setWorksheets] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);

  useEffect(() => { void loadAccounts(); }, []);

  async function loadAccounts() {
    try {
      const loaded = await listAccounts();
      setAccounts(loaded);
      setAccountId((current) => current || loaded[0]?.id || "");
    } catch (error) { onError(String(error)); }
  }

  async function addAccount() {
    try {
      setBusy(true);
      const account = await createAccount({ name: newAccountName, institution: "Canara Bank" });
      setAccounts((current) => [...current, account]);
      setAccountId(account.id);
      setNewAccountName("");
    } catch (error) { onError(String(error)); } finally { setBusy(false); }
  }

  async function chooseFile() {
    const selected = await open({ multiple: false, directory: false, filters: [{ name: "Statements", extensions: ["csv", "xlsx"] }] });
    const selectedPath = selectedCsvPath(selected);
    if (!selectedPath) return;
    setPath(selectedPath);
    const isXlsx = selectedPath.toLowerCase().endsWith(".xlsx"); setXlsx(isXlsx); setWorksheet(""); setWorksheets([]);
    setPreview(null);
    setResult(null);
    if (accountId) await requestPreview(selectedPath);
  }

  async function requestPreview(selectedPath = path, mapping?: CsvMapping) {
    if (!accountId || !selectedPath) return;
    try {
      setBusy(true);
      if (xlsx) { const response: XlsxPreview = await previewXlsxImport(accountId, selectedPath, worksheet || undefined, mapping); setPreview(response.preview); setWorksheets(response.worksheets); setWorksheet(response.selectedWorksheet); }
      else setPreview(await previewCsvImport(accountId, selectedPath, mapping));
      setResult(null);
    } catch (error) { onError(String(error)); } finally { setBusy(false); }
  }

  async function commit() {
    if (!preview || !canCommitImport(accountId, path, preview)) return;
    try {
      setBusy(true);
      const committed = xlsx ? await commitXlsxImport(accountId, path, worksheet, preview.mapping) : await commitCsvImport(accountId, path, preview.mapping);
      if (xlsx) { const refreshed = await previewXlsxImport(accountId, path, worksheet, preview.mapping); setPreview(refreshed.preview); }
      else setPreview(await previewCsvImport(accountId, path, preview.mapping));
      setResult(committed);
    } catch (error) { onError(String(error)); } finally { setBusy(false); }
  }

  const updateMapping = (field: keyof CsvMapping, value: string) => {
    if (!preview) return;
    const mapping = { ...preview.mapping, [field]: value || undefined };
    void requestPreview(path, mapping);
  };

  return <section className="import-screen" aria-labelledby="import-heading">
    <div className="page-heading"><p className="eyebrow">Local CSV import</p><h1 id="import-heading">Bring in a statement</h1><p>See what is new before anything reaches your local database. Rows you already imported are skipped.</p></div>
    {accounts.length === 0 ? <div className="panel"><h2>Create a local account first</h2><p>No bank credentials are requested or stored.</p><div className="inline-form"><input value={newAccountName} onChange={(event) => setNewAccountName(event.target.value)} placeholder="Daily account" aria-label="Account name" /><button onClick={() => void addAccount()} disabled={busy || !newAccountName.trim()}>Add Canara account</button></div></div> : <>
      <div className="panel import-controls"><label>Import to<select value={accountId} onChange={(event) => { setAccountId(event.target.value); setPreview(null); setResult(null); }}><option value="">Choose an account</option>{accounts.map((account) => <option key={account.id} value={account.id}>{account.name} · {account.institution}</option>)}</select></label><button onClick={() => void chooseFile()} disabled={busy || !accountId}>Choose CSV or XLSX</button>{path && <span className="file-name">Selected: {path.split("/").at(-1)}</span>}{xlsx && worksheets.length > 0 && <label>Worksheet<select value={worksheet} onChange={(event) => { setWorksheet(event.target.value); void requestPreview(path); }}>{worksheets.map((name) => <option key={name}>{name}</option>)}</select></label>}</div>
      {preview && <Preview preview={preview} busy={busy} onMappingChange={updateMapping} onRefresh={() => void requestPreview()} onCommit={() => void commit()} canCommit={canCommitImport(accountId, path, preview)} />}
      {result && <div className="panel success" role="status"><strong>{result.rowsImported === 0 ? "Nothing new to import." : "Import committed locally."}</strong><span>{result.rowsImported} new rows added · {result.rowsSkippedDuplicate} already in Ledger and skipped · {result.rowsRejected} rejected.</span></div>}
    </>}
    <ImportHistoryPanels onError={onError} refreshKey={result?.batchId ?? ""} onChanged={() => { setPreview(null); setResult(null); }} />
  </section>;
}

function Preview({ preview, busy, onMappingChange, onRefresh, onCommit, canCommit }: { preview: CsvPreview; busy: boolean; onMappingChange: (field: keyof CsvMapping, value: string) => void; onRefresh: () => void; onCommit: () => void; canCommit: boolean }) {
  const selects: [keyof CsvMapping, string][] = [["transactionDate", "Transaction date"], ["description", "Description"], ["debit", "Debit"], ["credit", "Credit"], ["reference", "Reference / UTR"]];
  return <div className="preview-stack"><div className="panel preview-summary"><div><p className="eyebrow">Preview · {preview.originalFilename}</p><h2>{preview.rowsNew} new of {preview.rowsSeen} rows</h2><p>{preview.periodStart && preview.periodEnd ? `${preview.periodStart} to ${preview.periodEnd}` : "No valid date range"}</p></div><div className="totals"><span>Already in Ledger <strong>{preview.rowsAlreadyImported}</strong></span><span>New debits <strong>{paise(preview.newDebitTotalPaise)}</strong></span><span>New credits <strong>{paise(preview.newCreditTotalPaise)}</strong></span><span>File debits <strong>{paise(preview.debitTotalPaise)}</strong></span><span>File credits <strong>{paise(preview.creditTotalPaise)}</strong></span></div></div><div className="panel"><h2>Confirm column mapping</h2><div className="mapping-grid">{selects.map(([field, label]) => <label key={field}>{label}<select value={preview.mapping[field] ?? ""} onChange={(event) => onMappingChange(field, event.target.value)}><option value="">Not mapped</option>{preview.headers.map((header) => <option key={header} value={header}>{header}</option>)}</select></label>)}</div><button className="secondary" onClick={onRefresh} disabled={busy}>Refresh preview</button></div><div className="panel notice"><strong>{preview.invalidRows.length} invalid rows</strong><span>{preview.invalidRows.length ? preview.invalidRows.slice(0, 4).map((issue) => `Row ${issue.rowNumber}: ${issue.reason}`).join(" · ") : "All rows currently validate."}</span><strong>{preview.rowsAlreadyImported} rows already in Ledger</strong><span>Rows imported earlier are skipped, so overlapping weekly statements only add what is new. Matching uses the reference number when there is one, otherwise date, direction, amount and description. Identical purchases on the same day are kept if the file has more of them than Ledger.</span></div><div className="commit-row"><span>{preview.rowsNew === 0 ? "Every valid row is already in Ledger, so there is nothing to import." : "Only the new rows are added. Invalid rows are excluded after this confirmation."}</span><button onClick={onCommit} disabled={busy || !canCommit}>{busy ? "Working…" : preview.rowsNew > 0 ? `Import ${preview.rowsNew} new rows` : "Nothing new to import"}</button></div></div>;
}
