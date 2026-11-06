import { useCallback, useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  archiveAccount,
  archiveCategory,
  backupDatabaseEncrypted,
  createCategory,
  getSettings,
  listAccounts,
  listCategories,
  renameAccount,
  renameCategory,
  restoreDatabaseEncrypted,
  setFirstDayOfMonth,
  type Account,
  type AppSettings,
  type CategoryRow
} from "../../app/commands";

type Props = { onError: (message: string) => void; onRestored?: () => void };

export function ManagementPanels({ onError, onRestored }: Props) {
  return (
    <>
      <PreferencesPanel onError={onError} />
      <AccountsPanel onError={onError} />
      <CategoriesPanel onError={onError} />
      <EncryptedBackupPanel onError={onError} onRestored={onRestored} />
    </>
  );
}

function PreferencesPanel({ onError }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [day, setDay] = useState("1");
  const [message, setMessage] = useState("");

  useEffect(() => {
    void getSettings()
      .then((next) => {
        setSettings(next);
        setDay(String(next.firstDayOfMonth));
      })
      .catch((error) => onError(String(error)));
  }, [onError]);

  const saveDay = async () => {
    try {
      await setFirstDayOfMonth(Number(day));
      setMessage("Saved. Overview months now start on this day.");
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Preferences</h2>
      <p>Currency is Indian rupees. Budgets always follow calendar months.</p>
      <div className="inline-form">
        <label>
          First day of month (1 to 28)
          <input type="number" min={1} max={28} value={day} onChange={(event) => setDay(event.target.value)} />
        </label>
        <button type="button" onClick={() => void saveDay()}>Save</button>
      </div>
      {message && <p className="success">{message}</p>}
      <dl className="detail-list">
        <div>
          <dt>Database</dt>
          <dd className="mono">{settings?.databasePath ?? "Unknown"}</dd>
        </div>
        <div>
          <dt>Backups</dt>
          <dd>Saved only where you choose when you create one. Ledger never creates backups on its own.</dd>
        </div>
      </dl>
    </div>
  );
}

function AccountsPanel({ onError }: Props) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);

  const load = useCallback(async () => {
    try {
      setAccounts(await listAccounts());
    } catch (error) {
      onError(String(error));
    }
  }, [onError]);
  useEffect(() => {
    void load();
  }, [load]);

  const rename = async () => {
    if (!editing) return;
    try {
      await renameAccount(editing.id, editing.name);
      setEditing(null);
      await load();
    } catch (error) {
      onError(String(error));
    }
  };

  const archive = async (account: Account) => {
    if (!window.confirm(`Archive ${account.name}? Its transactions stay in your records, but you can no longer import into it.`)) return;
    try {
      await archiveAccount(account.id);
      await load();
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Accounts</h2>
      {accounts.length === 0 ? (
        <p>No accounts yet. Add one from Import.</p>
      ) : (
        <ul className="category-summary">
          {accounts.map((account) => (
            <li key={account.id}>
              {editing?.id === account.id ? (
                <>
                  <input
                    aria-label="Account name"
                    value={editing.name}
                    onChange={(event) => setEditing({ id: account.id, name: event.target.value })}
                  />
                  <span className="button-group">
                    <button type="button" onClick={() => void rename()}>Save</button>
                    <button type="button" className="secondary" onClick={() => setEditing(null)}>Cancel</button>
                  </span>
                </>
              ) : (
                <>
                  <span>
                    <strong>{account.name}</strong>
                    <br />
                    <small>{account.institution}</small>
                  </span>
                  <span className="button-group">
                    <button type="button" className="secondary" onClick={() => setEditing({ id: account.id, name: account.name })}>Rename</button>
                    <button type="button" className="secondary" onClick={() => void archive(account)}>Archive</button>
                  </span>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function CategoriesPanel({ onError }: Props) {
  const [categories, setCategories] = useState<CategoryRow[]>([]);
  const [name, setName] = useState("");
  const [kind, setKind] = useState("expense");
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);

  const load = useCallback(async () => {
    try {
      setCategories(await listCategories());
    } catch (error) {
      onError(String(error));
    }
  }, [onError]);
  useEffect(() => {
    void load();
  }, [load]);

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action();
      await load();
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Categories</h2>
      <p>Built-in categories are fixed. Add your own for spending that does not fit them.</p>
      <div className="inline-form">
        <label>
          New category
          <input value={name} onChange={(event) => setName(event.target.value)} placeholder="Pet care" />
        </label>
        <label>
          Type
          <select value={kind} onChange={(event) => setKind(event.target.value)}>
            <option value="expense">Expense</option>
            <option value="income">Income</option>
          </select>
        </label>
        <button
          type="button"
          disabled={!name.trim()}
          onClick={() => void run(async () => { await createCategory({ name, kind }); setName(""); })}
        >
          Add category
        </button>
      </div>
      <ul className="category-summary">
        {categories.map((category) => (
          <li key={category.id}>
            {editing?.id === category.id ? (
              <>
                <input
                  aria-label="Category name"
                  value={editing.name}
                  onChange={(event) => setEditing({ id: category.id, name: event.target.value })}
                />
                <span className="button-group">
                  <button type="button" onClick={() => void run(async () => { await renameCategory(editing.id, editing.name); setEditing(null); })}>Save</button>
                  <button type="button" className="secondary" onClick={() => setEditing(null)}>Cancel</button>
                </span>
              </>
            ) : (
              <>
                <span>
                  <strong>{category.name}</strong>
                  <br />
                  <small>{category.kind}{category.isSystem ? " · built in" : ""}</small>
                </span>
                {!category.isSystem && (
                  <span className="button-group">
                    <button type="button" className="secondary" onClick={() => setEditing({ id: category.id, name: category.name })}>Rename</button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => {
                        if (window.confirm(`Archive ${category.name}? Its budgets are removed and rules using it go back to review. Past transactions keep it.`)) {
                          void run(() => archiveCategory(category.id));
                        }
                      }}
                    >
                      Archive
                    </button>
                  </span>
                )}
              </>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function EncryptedBackupPanel({ onError, onRestored }: Props) {
  const [passphrase, setPassphrase] = useState("");
  const [status, setStatus] = useState("");
  const ready = passphrase.length >= 8;

  const backup = async () => {
    const path = await save({ defaultPath: "Ledger backup.sqlite3.age", filters: [{ name: "Encrypted Ledger backup", extensions: ["age"] }] });
    if (!path) return;
    try {
      await backupDatabaseEncrypted(path, passphrase);
      setStatus("Encrypted backup created.");
    } catch (error) {
      onError(String(error));
    }
  };

  const restore = async () => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Encrypted Ledger backup", extensions: ["age"] }] });
    if (typeof path !== "string") return;
    if (!window.confirm("Restoring replaces all current Ledger data with the backup. Continue?")) return;
    try {
      await restoreDatabaseEncrypted(path, passphrase);
      setStatus("Encrypted backup restored.");
      onRestored?.();
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="panel">
      <h2>Encrypted backup</h2>
      <p>
        Protects a backup with a passphrase (age, scrypt). There is no recovery if you forget it, so store it somewhere safe.
      </p>
      <div className="inline-form">
        <label>
          Passphrase (8 or more characters)
          <input type="password" autoComplete="new-password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
        </label>
        <button type="button" disabled={!ready} onClick={() => void backup()}>Create encrypted backup</button>
        <button type="button" className="secondary" disabled={!passphrase} onClick={() => void restore()}>Restore encrypted backup</button>
      </div>
      {status && <p className="success">{status}</p>}
    </div>
  );
}
