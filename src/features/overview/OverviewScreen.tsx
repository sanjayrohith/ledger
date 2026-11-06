import { useCallback, useEffect, useState } from "react";
import {
  dismissRecurring,
  getOverviewSummary,
  recurringSuggestions,
  type OverviewSummary,
  type RecurringSuggestion
} from "../../app/commands";
import { AccountBars, DailyBars, Donut, MonthTable, MonthTimeline, WeekdayBars, monthName, money, toSlices } from "./charts";

const inr = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR", maximumFractionDigits: 0 });
const paise = (value: number) => inr.format(value / 100);
const formatStamp = (iso: string) => {
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? iso
    : date.toLocaleString("en-IN", { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
};
const signed = (value: number) => `${value >= 0 ? "+" : "−"}${paise(Math.abs(value))}`;

type Props = { onError: (error: string) => void };

export function OverviewScreen({ onError }: Props) {
  const [today] = useState(() => new Date().toLocaleDateString("en-CA"));
  const [selected, setSelected] = useState("all");
  const [summary, setSummary] = useState<OverviewSummary | null>(null);
  const [recurring, setRecurring] = useState<RecurringSuggestion[]>([]);

  useEffect(() => {
    void getOverviewSummary(today, { month: selected, rangeMonths: 1 })
      .then(setSummary)
      .catch((error) => onError(String(error)));
  }, [today, selected, onError]);

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

  const isAll = selected === "all";
  const months = summary.months ?? [];
  const coverage =
    summary.coverageStart && summary.coverageEnd
      ? `Statements cover ${summary.coverageStart} to ${summary.coverageEnd}.`
      : "No statements imported yet. Import one to see real totals.";
  const top = summary.categories[0];
  const change = summary.expensePaise - summary.previousExpensePaise;
  const scope = isAll ? "all months" : monthName(summary.periodLabel, true);
  const monthCount = Math.max(months.length, 1);
  const customDay = summary.firstDayOfMonth !== 1;
  const biggest = months.reduce<[string, number, number] | null>((best, row) => (!best || row[1] > best[1] ? row : best), null);

  return (
    <section className="overview-screen">
      <div className="page-heading">
        <h1>{isAll ? "All months" : monthName(summary.periodLabel, true)}</h1>
        <p>{coverage}</p>
        <p className="period-note">
          {summary.periodStart} to {summary.periodEnd}
          {customDay ? ` · months start on day ${summary.firstDayOfMonth}` : ""}
        </p>
      </div>

      <div className="panel">
        <div className="panel-head">
          <div>
            <h2>Month by month</h2>
            <p className="range-note">Each bar is the money you spent that month. Pick one to look closer.</p>
          </div>
          <button type="button" className={isAll ? "chip on" : "chip"} aria-pressed={isAll} onClick={() => setSelected("all")}>
            All months
          </button>
        </div>
        <MonthTimeline months={months} selected={selected} onSelect={setSelected} />
      </div>

      <div className="summary-cards">
        <Metric
          label="Spent"
          amount={summary.expensePaise}
          note={isAll ? `${paise(Math.round(summary.expensePaise / monthCount))} a month on average` : `${signed(change)} vs previous month`}
        />
        <Metric
          label="Income"
          amount={summary.incomePaise}
          note={isAll ? `${paise(Math.round(summary.incomePaise / monthCount))} a month on average` : `${signed(summary.incomePaise - summary.previousIncomePaise)} vs previous month`}
        />
        <Metric
          label="Net cash movement"
          amount={summary.netCashPaise}
          note={isAll ? (biggest ? `Biggest month: ${monthName(biggest[0], true)}` : undefined) : `${signed(summary.netCashPaise - (summary.previousIncomePaise - summary.previousExpensePaise))} vs previous month`}
        />
      </div>

      {summary.uncategorisedPaise > 0 && (
        <p className="notice">
          {paise(summary.uncategorisedPaise)} of money out in {scope} has no category yet, so it shows as Uncategorised.
          Add rules in Settings or edit rows in Transactions to sort it.
        </p>
      )}

      <div className="insight-grid wide">
        <div className="panel">
          <h2>Where spending went</h2>
          <p className="range-note">{scope}</p>
          <Donut slices={toSlices(summary.categories)} centreLabel="money out" />
        </div>
        <div className="panel">
          <h2>By account</h2>
          <p className="range-note">{scope}</p>
          <AccountBars rows={summary.accounts ?? []} />
        </div>
      </div>

      <div className="insight-grid wide">
        <div className="panel">
          {isAll ? (
            <>
              <h2>Every month</h2>
              <p className="range-note">Select a row to open that month</p>
              <MonthTable months={months} onSelect={setSelected} />
            </>
          ) : (
            <>
              <h2>Day by day</h2>
              <p className="range-note">{summary.periodStart} to {summary.periodEnd}</p>
              <DailyBars days={summary.daily ?? []} start={summary.periodStart} end={summary.periodEnd} />
            </>
          )}
        </div>
        <div className="stack">
          <div className="panel">
            <h2>Busiest weekdays</h2>
            <p className="range-note">{scope}</p>
            <WeekdayBars values={summary.weekdays ?? []} />
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
                  <strong>{top[0]}</strong> is your largest category in {scope} at {paise(top[1])}.
                </li>
              )}
              {!isAll && <li>Spending is {change >= 0 ? "up" : "down"} {paise(Math.abs(change))} versus the previous month.</li>}
              {isAll && biggest && <li>{monthName(biggest[0], true)} was your heaviest month at {paise(biggest[1])}.</li>}
              <li>Use Budgets to set a limit before reducing a category.</li>
            </ul>
          )}
          <p className="accessible-summary">These are observations from imported transactions, not financial advice.</p>
        </div>
        </div>
      </div>

      <div className="panel">
        <h2>Top merchants</h2>
        <p className="range-note">{scope}</p>
        <Bars values={summary.merchants} />
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
