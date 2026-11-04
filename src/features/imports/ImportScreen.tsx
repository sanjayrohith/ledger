import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import type { Account, CommitResult, CsvMapping, CsvPreview } from "../../app/commands";
import { commitCsvImport, createAccount, listAccounts, previewCsvImport } from "../../app/commands";
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
    const selected = await open({ multiple: false, directory: false, filters: [{ name: "CSV statements", extensions: ["csv"] }] });
    const selectedPath = selectedCsvPath(selected);
    if (!selectedPath) return;
    setPath(selectedPath);
    setPreview(null);
    setResult(null);
    if (accountId) await requestPreview(selectedPath);
  }

  async function requestPreview(selectedPath = path, mapping?: CsvMapping) {
    if (!accountId || !selectedPath) return;
    try {
      setBusy(true);
      setPreview(await previewCsvImport(accountId, selectedPath, mapping));
      setResult(null);
    } catch (error) { onError(String(error)); } finally { setBusy(false); }
  }

  async function commit() {
    if (!preview || !canCommitImport(accountId, path, preview)) return;
    try {
      setBusy(true);
      const committed = await commitCsvImport(accountId, path, preview.mapping);
      const refreshed = await previewCsvImport(accountId, path, preview.mapping);
      setPreview(refreshed);
      setResult(committed);
    } catch (error) { onError(String(error)); } finally { setBusy(false); }
  }

  const updateMapping = (field: keyof CsvMapping, value: string) => {
    if (!preview) return;
    const mapping = { ...preview.mapping, [field]: value || undefined };
    void requestPreview(path, mapping);
  };

  return <section className="import-screen" aria-labelledby="import-heading">
    <div className="page-heading"><p className="eyebrow">Local CSV import</p><h1 id="import-heading">Bring in a statement, deliberately.</h1><p>Preview validation and duplicate candidates before anything reaches your local database.</p></div>
    {accounts.length === 0 ? <div className="panel"><h2>Create a local account first</h2><p>No bank credentials are requested or stored.</p><div className="inline-form"><input value={newAccountName} onChange={(event) => setNewAccountName(event.target.value)} placeholder="e.g. Daily account" aria-label="Account name" /><button onClick={() => void addAccount()} disabled={busy || !newAccountName.trim()}>Add Canara account</button></div></div> : <>
      <div className="panel import-controls"><label>Import to<select value={accountId} onChange={(event) => { setAccountId(event.target.value); setPreview(null); setResult(null); }}><option value="">Choose an account</option>{accounts.map((account) => <option key={account.id} value={account.id}>{account.name} · {account.institution}</option>)}</select></label><button onClick={() => void chooseFile()} disabled={busy || !accountId}>Choose CSV file</button>{path && <span className="file-name">Selected: {path.split("/").at(-1)}</span>}</div>
      {preview && <Preview preview={preview} busy={busy} onMappingChange={updateMapping} onRefresh={() => void requestPreview()} onCommit={() => void commit()} canCommit={canCommitImport(accountId, path, preview)} />}
      {result && <div className="panel success" role="status"><strong>Import committed locally.</strong><span>{result.rowsImported} rows imported · {result.rowsRejected} rejected · {result.rowsSkippedDuplicate} trusted duplicates skipped.</span></div>}
    </>}
  </section>;
}

function Preview({ preview, busy, onMappingChange, onRefresh, onCommit, canCommit }: { preview: CsvPreview; busy: boolean; onMappingChange: (field: keyof CsvMapping, value: string) => void; onRefresh: () => void; onCommit: () => void; canCommit: boolean }) {
  const selects: [keyof CsvMapping, string][] = [["transactionDate", "Transaction date"], ["description", "Description"], ["debit", "Debit"], ["credit", "Credit"], ["reference", "Reference / UTR"]];
  return <div className="preview-stack"><div className="panel preview-summary"><div><p className="eyebrow">Preview · {preview.originalFilename}</p><h2>{preview.validRows} valid of {preview.rowsSeen} rows</h2><p>{preview.periodStart && preview.periodEnd ? `${preview.periodStart} to ${preview.periodEnd}` : "No valid date range"}</p></div><div className="totals"><span>Debits <strong>{paise(preview.debitTotalPaise)}</strong></span><span>Credits <strong>{paise(preview.creditTotalPaise)}</strong></span></div></div><div className="panel"><h2>Confirm column mapping</h2><div className="mapping-grid">{selects.map(([field, label]) => <label key={field}>{label}<select value={preview.mapping[field] ?? ""} onChange={(event) => onMappingChange(field, event.target.value)}><option value="">Not mapped</option>{preview.headers.map((header) => <option key={header} value={header}>{header}</option>)}</select></label>)}</div><button className="secondary" onClick={onRefresh} disabled={busy}>Refresh preview</button></div><div className="panel notice"><strong>{preview.invalidRows.length} invalid rows</strong><span>{preview.invalidRows.length ? preview.invalidRows.slice(0, 4).map((issue) => `Row ${issue.rowNumber}: ${issue.reason}`).join(" · ") : "All rows currently validate."}</span><strong>{preview.duplicateCandidates.length} duplicate candidates</strong><span>Trusted reference matches are skipped on commit. Fingerprint matches remain visible for review because repeated identical purchases can be legitimate.</span></div><div className="commit-row"><span>Commit accepts only the valid rows above. Invalid rows are excluded only after this confirmation.</span><button onClick={onCommit} disabled={busy || !canCommit}>{busy ? "Working…" : "Confirm and import"}</button></div></div>;
}
