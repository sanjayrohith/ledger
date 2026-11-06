import { useCallback, useEffect, useState } from "react";
import {
  dismissRecurring,
  getOverviewSummary,
  recurringSuggestions,
  type OverviewSummary,
  type RecurringSuggestion
} from "../../app/commands";

const inr = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR", maximumFractionDigits: 2 });
const paise = (value: number) => inr.format(value / 100);
const signed = (value: number) => `${value >= 0 ? "+" : "−"}${paise(Math.abs(value))}`;

const RANGES = [
  { months: 1, label: "This period" },
  { months: 3, label: "Last 3 periods" },
  { months: 6, label: "Last 6 periods" },
  { months: 12, label: "Last 12 periods" }
] as const;

type Props = { onError: (error: string) => void };

export function OverviewScreen({ onError }: Props) {
  const [today] = useState(() => new Date().toLocaleDateString("en-CA"));
  const [month, setMonth] = useState("");
  const [rangeMonths, setRangeMonths] = useState(1);
  const [summary, setSummary] = useState<OverviewSummary | null>(null);
  const [recurring, setRecurring] = useState<RecurringSuggestion[]>([]);

  useEffect(() => {
    void getOverviewSummary(today, { month: month || undefined, rangeMonths })
      .then(setSummary)
      .catch((error) => onError(String(error)));
  }, [today, month, rangeMonths, onError]);

  const loadRecurring = useCallback(() => {
    void recurringSuggestions().then(setRecurring).catch((error) => onError(String(error)));
  }, [onError]);
  useEffect(loadRecurring, [loadRecurring]);

  if (!summary) {
    return (
      <section className="welcome-card">
        <h1>Overview</h1>
        <p className="detail">Loading your local summary…</p>
      </section>
    );
  }

  const coverage =
    summary.coverageStart && summary.coverageEnd
      ? `Imported statements cover ${summary.coverageStart} to ${summary.coverageEnd}. This may be partial.`
      : "No statement coverage yet. Import a statement to see real totals.";
  const top = summary.categories[0];
  const change = summary.expensePaise - summary.previousExpensePaise;
  const rangeLabel = rangeMonths === 1 ? "this period" : `the last ${rangeMonths} periods`;
  const customDay = summary.firstDayOfMonth !== 1;

  return (
    <section className="overview-screen">
      <div className="page-heading">
        <p className="eyebrow">Monthly overview</p>
        <h1>{summary.periodLabel}</h1>
        <p>{coverage}</p>
        <p className="period-note">
          {summary.periodStart} to {summary.periodEnd}
          {customDay ? ` · months start on day ${summary.firstDayOfMonth}` : ""}
        </p>
      </div>

      <div className="toolbar">
        <label>
          Month
          <input type="month" value={month || summary.periodLabel} onChange={(event) => setMonth(event.target.value)} />
        </label>
        <label>
          Range
          <select value={rangeMonths} onChange={(event) => setRangeMonths(Number(event.target.value))}>
            {RANGES.map((range) => (
              <option key={range.months} value={range.months}>{range.label}</option>
            ))}
          </select>
        </label>
        {month && (
          <button type="button" className="secondary" onClick={() => setMonth("")}>Current period</button>
        )}
      </div>

      <div className="summary-cards">
        <Metric label="Spent" amount={summary.expensePaise} note={`${signed(change)} vs previous month`} />
        <Metric label="Income" amount={summary.incomePaise} note={`${signed(summary.incomePaise - summary.previousIncomePaise)} vs previous month`} />
        <Metric label="Net cash movement" amount={summary.netCashPaise} note={`${signed(summary.netCashPaise - (summary.previousIncomePaise - summary.previousExpensePaise))} vs previous month`} />
      </div>

      <div className="insight-grid wide">
        <div className="panel">
          <h2>Spending trend</h2>
          <Trend points={summary.trend} />
          <p className="accessible-summary">
            Last {Math.max(6, rangeMonths)} periods of spending. Transfers and unknown transactions are excluded.
          </p>
        </div>
        <div className="panel">
          <h2>Where spending went</h2>
          <p className="range-note">{rangeLabel}</p>
          <Bars values={summary.categories} />
        </div>
      </div>

      <div className="insight-grid">
        <div className="panel">
          <h2>Top merchants</h2>
          <p className="range-note">{rangeLabel}</p>
          <Bars values={summary.merchants} />
        </div>
        <div className="panel">
          <h2>Observations</h2>
          {summary.budgetAlerts.length > 0 ? (
            <ul className="insight-list">
              {summary.budgetAlerts.map(([name, budget, spent]) => (
                <li key={name}>
                  <strong>{name}</strong> is {paise(spent - budget)} over its {paise(budget)} budget.
                </li>
              ))}
            </ul>
          ) : (
            <ul className="insight-list">
              {top && (
                <li>
                  <strong>{top[0]}</strong> is your largest classified category at {paise(top[1])}.
                </li>
              )}
              <li>Spending is {change >= 0 ? "up" : "down"} {paise(Math.abs(change))} versus the previous month.</li>
              <li>Use Budgets to set a limit before reducing a category.</li>
            </ul>
          )}
          <p className="accessible-summary">These are observations from imported, classified transactions—not financial advice.</p>
        </div>
      </div>

      {recurring.length > 0 && (
        <div className="panel">
          <h2>Possible recurring payments</h2>
          <p>
            Charged in three or more months for a similar amount. These are suggestions only. Nothing changes unless you act on it.
          </p>
          <ul className="category-summary">
            {recurring.map((item) => (
              <li key={item.merchantKey}>
                <span>
                  <strong>{item.merchant}</strong>
                  <br />
                  <small>{item.monthsSeen} months · last on {item.lastSeen}</small>
                </span>
                <span className="recurring-actions">
                  <span className="mono">{paise(item.typicalAmountPaise)}</span>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void dismissRecurring(item.merchantKey).then(loadRecurring).catch((error) => onError(String(error)))}
                  >
                    Not recurring
                  </button>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="panel coverage">
        <strong>{summary.unreviewedCount} transactions need review</strong>
        <span>{summary.lastImportedAt ? `Last import: ${summary.lastImportedAt}` : "No imports recorded yet"}</span>
      </div>
    </section>
  );
}

function Metric({ label, amount, note }: { label: string; amount: number; note?: string }) {
  return (
    <div className="panel metric">
      <span>{label}</span>
      <strong>{paise(amount)}</strong>
      {note && <small>{note}</small>}
    </div>
  );
}

function Bars({ values }: { values: [string, number][] }) {
  const max = Math.max(...values.map(([, value]) => value), 1);
  if (values.length === 0) return <p>No classified spending yet.</p>;
  return (
    <ul className="visual-list">
      {values.map(([name, value]) => (
        <li key={name}>
          <div>
            <span>{name}</span>
            <strong>{paise(value)}</strong>
          </div>
          <div className="progress" aria-label={`${name}: ${paise(value)}`}>
            <span style={{ width: `${Math.round((value / max) * 100)}%` }} />
          </div>
        </li>
      ))}
    </ul>
  );
}

function Trend({ points }: { points: [string, number][] }) {
  const max = Math.max(...points.map(([, value]) => value), 1);
  if (points.length === 0) return <p>No confirmed spending in this range.</p>;
  const label = (month: string) => (points.length > 6 ? month.slice(2) : month.slice(5));
  return (
    <div className="trend" role="img" aria-label={points.map(([month, value]) => `${month}: ${paise(value)}`).join(", ")}>
      {points.map(([month, value]) => (
        <div key={month}>
          <span style={{ height: `${Math.max(5, Math.round((value / max) * 100))}%` }} />
          <small>{label(month)}</small>
        </div>
      ))}
    </div>
  );
}
