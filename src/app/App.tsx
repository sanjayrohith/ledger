import { useState } from "react";
import { ImportScreen } from "../features/imports/ImportScreen";
import { TransactionsScreen } from "../features/transactions/TransactionsScreen";
import { OverviewScreen } from "../features/overview/OverviewScreen";
import { BudgetsScreen } from "../features/budgets/BudgetsScreen";
import { RulesScreen } from "../features/settings/RulesScreen";

export const appCopy = {
  eyebrow: "Private by design",
  title: "A clearer view of your money, kept on this Mac.",
  detail:
    "Your local Ledger database is ready. No accounts are connected, and no financial data has been created.",
  status: "Offline local storage ready"
} as const;

const navigation = ["Overview", "Import", "Transactions", "Budgets", "Settings"] as const;

export function App() {
  const [screen, setScreen] = useState<(typeof navigation)[number]>("Overview");
  const [error, setError] = useState("");
  return (
    <main className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand" aria-label="Ledger">
          <span className="brand-mark" aria-hidden="true">L</span>
          <span>Ledger</span>
        </div>
        <nav>
          {navigation.map((item) => (
            <button className={item === screen ? "nav-item active" : "nav-item"} onClick={() => { setScreen(item); setError(""); }} key={item}>
              {item}
            </button>
          ))}
        </nav>
        <p className="sidebar-note">Local-first finance</p>
      </aside>

      <section className="content" aria-labelledby="welcome-heading">
        <div className="topbar">
          <span className="window-label">{screen}</span>
          <span className="offline-badge">Offline</span>
        </div>
        {error && <div className="error-banner" role="alert">{error}</div>}
        {screen === "Import" ? <ImportScreen onError={setError} /> : screen === "Transactions" ? <TransactionsScreen onError={setError} /> : screen === "Overview" ? <OverviewScreen onError={setError} /> : screen === "Budgets" ? <BudgetsScreen onError={setError} /> : screen === "Settings" ? <RulesScreen onError={setError} /> : <div className="welcome-card">
          <p className="eyebrow">{appCopy.eyebrow}</p>
          <h1 id="welcome-heading">{appCopy.title}</h1>
          <p className="detail">{appCopy.detail}</p>
          <div className="status-row">
            <span className="status-dot" aria-hidden="true" />
            <span>{appCopy.status}</span>
          </div>
        </div>}
      </section>
    </main>
  );
}
