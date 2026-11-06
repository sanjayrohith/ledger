import { useCallback, useEffect, useState } from "react";
import {
  deleteImportBatch,
  deleteTransactionsInRange,
  listAccounts,
  listImportBatches,
  rangeSummary,
  type Account,
  type ImportBatchSummary,
  type RangeSummary
} from "../../app/commands";

const money = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" });
const rupees = (paise: number) => money.format(paise / 100);

type Props = { onError: (message: string) => void; refreshKey: string; onChanged: () => void };

export function ImportHistoryPanels(props: Props) {
  return (
    <>
      <ImportHistory {...props} />
      <DeleteByRange {...props} />
    </>
  );
}

function ImportHistory({ onError, refreshKey, onChanged }: Props) {
  const [batches, setBatches] = useState<ImportBatchSummary[]>([]);
  const [message, setMessage] = useState("");

  const load = useCallback(async () => {
    try {
      setBatches(await listImportBatches());
    } catch (error) {
      onError(String(error));
    }
  }, [onError]);
  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  const remove = async (batch: ImportBatchSummary) => {
    const edits = batch.editedByYou > 0 ? `, including ${batch.editedByYou} you reviewed or categorised (those edits are lost)` : "";
    const warning =
      `Delete the import of "${batch.filename}"?\n\nThis removes the ${batch.transactionsRemaining} transactions it added${edits}.\n` +
      "If a later file repeated some of them, upload that later file again to bring them back.";
    if (!window.confirm(warning)) return;
    try {
      const deleted = await deleteImportBatch(batch.id);
      setMessage(`Deleted the import and ${deleted} transactions. You can upload the file again.`);
      await load();
      onChanged();
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Import history</h2>
      <p>Each upload adds only transactions Ledger did not already have. Delete an upload to remove what it added.</p>
      {message && <p className="success">{message}</p>}
      {batches.length === 0 ? (
        <p>No imports yet.</p>
      ) : (
        <ul className="category-summary">
          {batches.map((batch) => (
            <li key={batch.id}>
              <span>
                <strong>{batch.filename}</strong>
                <br />
                <small>
                  {batch.accountName} · imported {batch.importedAt.slice(0, 10)}
                  {batch.periodStart && batch.periodEnd ? ` · covers ${batch.periodStart} to ${batch.periodEnd}` : ""}
                </small>
                <br />
                <small>
                  {batch.rowsImported} added · {batch.rowsSkippedDuplicate} already in Ledger · {batch.transactionsRemaining} remain
                </small>
              </span>
              <button type="button" className="secondary" onClick={() => void remove(batch)}>Delete import</button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function DeleteByRange({ onError, refreshKey, onChanged }: Props) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountId, setAccountId] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [summary, setSummary] = useState<RangeSummary | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    void listAccounts().then(setAccounts).catch((error) => onError(String(error)));
  }, [onError]);

  const ready = Boolean(from && to && from <= to);
  useEffect(() => {
    if (!ready) {
      setSummary(null);
      return;
    }
    void rangeSummary(accountId || undefined, from, to)
      .then(setSummary)
      .catch((error) => onError(String(error)));
  }, [accountId, from, to, ready, refreshKey, onError]);

  const remove = async () => {
    if (!summary || summary.count === 0) return;
    const edits = summary.editedByYou > 0 ? ` ${summary.editedByYou} of them were reviewed or categorised by you, and those edits are lost.` : "";
    const scope = accountId ? accounts.find((account) => account.id === accountId)?.name : "all accounts";
    if (!window.confirm(`Delete ${summary.count} transactions from ${from} to ${to} in ${scope}?${edits}\n\nUploading a statement that covers these dates again will add them back.`)) return;
    try {
      const deleted = await deleteTransactionsInRange(accountId || undefined, from, to);
      setMessage(`Deleted ${deleted} transactions from ${from} to ${to}.`);
      setSummary(await rangeSummary(accountId || undefined, from, to));
      onChanged();
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Delete by date range</h2>
      <p>Remove every transaction between two dates, then upload a corrected statement.</p>
      <div className="inline-form">
        <label>
          Account
          <select value={accountId} onChange={(event) => setAccountId(event.target.value)}>
            <option value="">All accounts</option>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>{account.name}</option>
            ))}
          </select>
        </label>
        <label>
          From
          <input type="date" value={from} onChange={(event) => { setFrom(event.target.value); setMessage(""); }} />
        </label>
        <label>
          To
          <input type="date" value={to} onChange={(event) => { setTo(event.target.value); setMessage(""); }} />
        </label>
        <button type="button" disabled={!summary || summary.count === 0} onClick={() => void remove()}>
          {summary && summary.count > 0 ? `Delete ${summary.count} transactions` : "Delete"}
        </button>
      </div>
      {from && to && from > to && <p className="over-budget">The start date is after the end date.</p>}
      {summary && (
        <p>
          {summary.count === 0
            ? "No transactions in this range."
            : `${summary.count} transactions · ${rupees(summary.debitTotalPaise)} out · ${rupees(summary.creditTotalPaise)} in${summary.editedByYou > 0 ? ` · ${summary.editedByYou} reviewed or categorised by you` : ""}`}
        </p>
      )}
      {message && <p className="success">{message}</p>}
    </div>
  );
}
