import { useEffect,useState } from "react";
import {getOverviewSummary,type OverviewSummary} from "../../app/commands";

const inr=new Intl.NumberFormat("en-IN",{style:"currency",currency:"INR",maximumFractionDigits:2});
const paise=(value:number)=>inr.format(value/100);
export function OverviewScreen({onError}:{onError:(error:string)=>void}){
 const [summary,setSummary]=useState<OverviewSummary|null>(null); const [today]=useState(()=>new Date().toLocaleDateString("en-CA"));
 useEffect(()=>{void getOverviewSummary(today).then(setSummary).catch(e=>onError(String(e)));},[today]);
 if(!summary)return <section className="welcome-card"><p className="eyebrow">Overview</p><h1>Your money, in context.</h1><p className="detail">Loading your local summary…</p></section>;
 const coverage=summary.coverageStart&&summary.coverageEnd?`Imported statements cover ${summary.coverageStart} to ${summary.coverageEnd}. This may be partial.`:"No statement coverage yet. Import a statement to see real totals.";
 return <section className="overview-screen"><div className="page-heading"><p className="eyebrow">Monthly overview</p><h1>{summary.periodStart.slice(0,7)}</h1><p>{coverage}</p></div><div className="summary-cards"><Metric label="Spent" amount={summary.expensePaise}/><Metric label="Income" amount={summary.incomePaise}/><Metric label="Net cash movement" amount={summary.netCashPaise}/></div><div className="panel"><h2>Compared with previous month</h2><p>Spending {summary.expensePaise-summary.previousExpensePaise>=0?"up":"down"} {paise(Math.abs(summary.expensePaise-summary.previousExpensePaise))} · income {summary.incomePaise-summary.previousIncomePaise>=0?"up":"down"} {paise(Math.abs(summary.incomePaise-summary.previousIncomePaise))}</p></div><div className="panel"><h2>Spending by category</h2>{summary.categories.length===0?<p>No confirmed expenses in this period.</p>:<ul className="category-summary">{summary.categories.map(([name,value])=><li key={name}><span>{name}</span><strong>{paise(value)}</strong></li>)}</ul>}<p className="accessible-summary">Category spending totals are calculated from classified expenses and fees. Transfers and unknown transactions are excluded.</p></div><div className="panel coverage"><strong>{summary.unreviewedCount} transactions need review</strong><span>{summary.lastImportedAt?`Last import: ${summary.lastImportedAt}`:"No imports recorded yet"}</span></div></section>
}
function Metric({label,amount}:{label:string;amount:number}){return <div className="panel metric"><span>{label}</span><strong>{paise(amount)}</strong></div>}
