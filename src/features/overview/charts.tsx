import { useState } from "react";

const inr = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR", maximumFractionDigits: 0 });
export const money = (paise: number) => inr.format(paise / 100);

// Muted categorical palette. Each hue only ever means one category in the donut.
export const SERIES = ["#78a88b", "#7d9bc4", "#c9a66b", "#b58aa8", "#6fb0b5", "#c4837a", "#9a9f7c"];
const OTHER = "#5a5f5c";

export type Slice = { name: string; value: number; color: string };

export function toSlices(values: [string, number][], max = 6): Slice[] {
  const head = values.slice(0, max).map(([name, value], index) => ({ name, value, color: SERIES[index % SERIES.length] }));
  const rest = values.slice(max).reduce((sum, [, value]) => sum + value, 0);
  return rest > 0 ? [...head, { name: "Everything else", value: rest, color: OTHER }] : head;
}

export function Donut({ slices, centreLabel }: { slices: Slice[]; centreLabel: string }) {
  const [active, setActive] = useState<number | null>(null);
  const total = slices.reduce((sum, slice) => sum + slice.value, 0);
  if (total <= 0) return <p className="detail">No spending in this range.</p>;
  const radius = 54;
  const circumference = 2 * Math.PI * radius;
  let offset = 0;
  const shown = active === null ? null : slices[active];
  return (
    <div className="donut-wrap">
      <svg viewBox="0 0 140 140" className="donut" role="img" aria-label={slices.map((s) => `${s.name}: ${money(s.value)}`).join(", ")}>
        <circle cx="70" cy="70" r={radius} fill="none" stroke="var(--surface-raised)" strokeWidth="18" />
        {slices.map((slice, index) => {
          const length = (slice.value / total) * circumference;
          const gap = slices.length > 1 ? 1.5 : 0;
          const element = (
            <circle
              key={slice.name}
              cx="70"
              cy="70"
              r={radius}
              fill="none"
              stroke={slice.color}
              strokeWidth={active === index ? 21 : 18}
              strokeDasharray={`${Math.max(length - gap, 0)} ${circumference}`}
              strokeDashoffset={-offset}
              transform="rotate(-90 70 70)"
              opacity={active === null || active === index ? 1 : 0.4}
              onMouseEnter={() => setActive(index)}
              onMouseLeave={() => setActive(null)}
            />
          );
          offset += length;
          return element;
        })}
        <text x="70" y="66" textAnchor="middle" className="donut-figure">{money(shown ? shown.value : total)}</text>
        <text x="70" y="84" textAnchor="middle" className="donut-caption">
          {shown ? `${Math.round((shown.value / total) * 100)}% · ${shown.name.slice(0, 16)}` : centreLabel}
        </text>
      </svg>
      <ul className="legend">
        {slices.map((slice, index) => (
          <li key={slice.name} onMouseEnter={() => setActive(index)} onMouseLeave={() => setActive(null)} className={active === index ? "on" : ""}>
            <i style={{ background: slice.color }} />
            <span>{slice.name}</span>
            <small className="mono">{Math.round((slice.value / total) * 100)}%</small>
            <strong className="mono">{money(slice.value)}</strong>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Horizontal paired bars: money out (accent) and money in (neutral) for each account. */
export function AccountBars({ rows }: { rows: [string, number, number][] }) {
  if (rows.length === 0) return <p className="detail">No accounts with activity in this range.</p>;
  const max = Math.max(...rows.flatMap(([, out, inn]) => [out, inn]), 1);
  return (
    <ul className="account-bars">
      {rows.map(([name, out, inn]) => (
        <li key={name}>
          <div className="account-head"><span>{name}</span></div>
          <div className="pair">
            <span className="pair-label">Out</span>
            <div className="bar"><span className="out" style={{ width: `${Math.max(2, (out / max) * 100)}%` }} /></div>
            <strong className="mono">{money(out)}</strong>
          </div>
          <div className="pair">
            <span className="pair-label">In</span>
            <div className="bar"><span className="in" style={{ width: `${Math.max(2, (inn / max) * 100)}%` }} /></div>
            <strong className="mono">{money(inn)}</strong>
          </div>
        </li>
      ))}
    </ul>
  );
}

/** One bar per day of the period, with the biggest day highlighted. */
export function DailyBars({ days, start, end }: { days: [string, number][]; start: string; end: string }) {
  const byDate = new Map(days);
  const all: [string, number][] = [];
  for (let cursor = new Date(`${start}T00:00:00Z`); cursor <= new Date(`${end}T00:00:00Z`) && all.length < 62; cursor = new Date(cursor.getTime() + 86_400_000)) {
    const key = cursor.toISOString().slice(0, 10);
    all.push([key, byDate.get(key) ?? 0]);
  }
  const max = Math.max(...all.map(([, value]) => value), 1);
  if (days.length === 0) return <p className="detail">No spending in this period.</p>;
  return (
    <div className="daily" role="img" aria-label={`Daily spending, ${start} to ${end}`}>
      {all.map(([date, value]) => (
        <div key={date} title={`${date}: ${money(value)}`}>
          <span className={value === max ? "peak" : ""} style={{ height: `${value === 0 ? 2 : Math.max(4, (value / max) * 100)}%` }} />
        </div>
      ))}
    </div>
  );
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
export function WeekdayBars({ values }: { values: [number, number][] }) {
  const map = new Map(values);
  const rows = [1, 2, 3, 4, 5, 6, 0].map((d) => ({ d, value: map.get(d) ?? 0 }));
  const max = Math.max(...rows.map((r) => r.value), 1);
  if (values.length === 0) return <p className="detail">No spending in this range.</p>;
  return (
    <div className="weekday" role="img" aria-label={rows.map((r) => `${DAYS[r.d]}: ${money(r.value)}`).join(", ")}>
      {rows.map(({ d, value }) => (
        <div key={d} title={money(value)}>
          <span className={value === max ? "peak" : ""} style={{ height: `${Math.max(3, (value / max) * 100)}%` }} />
          <small>{DAYS[d]}</small>
        </div>
      ))}
    </div>
  );
}

const compact = (paise: number) => {
  const rupees = paise / 100;
  if (rupees >= 100000) return `${(rupees / 100000).toFixed(1)}L`;
  if (rupees >= 1000) return `${Math.round(rupees / 1000)}k`;
  return String(Math.round(rupees));
};
const SHORT = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const monthName = (label: string, long = false) => {
  const [year, month] = label.split("-").map(Number);
  if (!year || !month) return label;
  return long ? `${SHORT[month - 1]} ${year}` : `${SHORT[month - 1]} ’${String(year).slice(2)}`;
};

/** One clickable column per month. Bar = money out, tick = money in. */
export function MonthTimeline({
  months,
  selected,
  onSelect
}: {
  months: [string, number, number][];
  selected: string;
  onSelect: (label: string) => void;
}) {
  const max = Math.max(...months.map(([, out]) => out), 1);
  if (months.length === 0) return <p className="detail">Import a statement and its months will appear here.</p>;
  return (
    <div className="timeline" role="group" aria-label="Choose a month">
      {months.map(([label, out, inn]) => (
        <button
          type="button"
          key={label}
          className={selected === label ? "month-col on" : "month-col"}
          aria-pressed={selected === label}
          aria-label={`${monthName(label, true)}: spent ${money(out)}, received ${money(inn)}`}
          onClick={() => onSelect(label)}
        >
          <span className="month-plot">
            <span className="month-out" style={{ height: `${Math.max(2, (out / max) * 85)}%` }}>
              <em>{compact(out)}</em>
            </span>
          </span>
          <small>{monthName(label)}</small>
        </button>
      ))}
    </div>
  );
}

export function MonthTable({
  months,
  onSelect
}: {
  months: [string, number, number][];
  onSelect: (label: string) => void;
}) {
  const rows = [...months].reverse();
  return (
    <table className="month-table">
      <thead>
        <tr><th>Month</th><th>Spent</th><th>Received</th><th>Net</th></tr>
      </thead>
      <tbody>
        {rows.map(([label, out, inn], index) => {
          const previous = rows[index + 1];
          const delta = previous ? out - previous[1] : null;
          return (
            <tr key={label} onClick={() => onSelect(label)}>
              <td><button type="button" className="link" onClick={() => onSelect(label)}>{monthName(label, true)}</button></td>
              <td className="mono">
                {money(out)}
                {delta !== null && delta !== 0 && <small className={delta > 0 ? "up" : "down"}>{delta > 0 ? "+" : "−"}{money(Math.abs(delta))}</small>}
              </td>
              <td className="mono">{money(inn)}</td>
              <td className="mono">{inn - out < 0 ? "−" : "+"}{money(Math.abs(inn - out))}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
