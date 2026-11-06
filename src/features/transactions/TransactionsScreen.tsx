import { useCallback, useEffect, useState } from "react";
import {
  editTransaction,
  listAccounts,
  listCategories,
  listTransactions,
  type Account,
  type CategoryRow,
  type TransactionFilters,
  type TransactionItem,
  type TransactionPage
} from "../../app/commands";

const PAGE_SIZE = 50;
const currency = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR" });

type Props = { onError: (message: string) => void };

export function TransactionsScreen({ onError }: Props) {
  const [page, setPage] = useState<TransactionPage>({ items: [], total: 0 });
  const [loading, setLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  const [searchText, setSearchText] = useState("");
  const [filters, setFilters] = useState<TransactionFilters>({});
  const [reviewOnly, setReviewOnly] = useState(false);
  const [selected, setSelected] = useState<TransactionItem | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<CategoryRow[]>([]);

  useEffect(() => {
    void Promise.all([listAccounts(), listCategories()])
      .then(([nextAccounts, nextCategories]) => {
        setAccounts(nextAccounts);
        setCategories(nextCategories);
      })
      .catch((error) => onError(String(error)));
  }, [onError]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const next = await listTransactions({ ...filters, reviewOnly, limit: PAGE_SIZE, offset });
      setPage(next);
      setSelected((current) => next.items.find((item) => item.id === current?.id) ?? null);
    } catch (error) {
      onError(String(error));
    } finally {
      setLoading(false);
    }
  }, [filters, reviewOnly, offset, onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const setFilter = (patch: TransactionFilters) => {
    setOffset(0);
    setFilters((current) => ({ ...current, ...patch }));
  };

  const hasFilters = Object.values(filters).some(Boolean) || reviewOnly;
  const clearFilters = () => {
    setOffset(0);
    setFilters({});
    setSearchText("");
    setReviewOnly(false);
  };

  const save = async () => {
    if (!selected) return;
    try {
      await editTransaction(selected.id, {
        merchantOverride: selected.merchantOverride ?? "",
        categoryId: selected.categoryId ?? "",
        isReviewed: selected.isReviewed
      });
      await load();
    } catch (error) {
      onError(String(error));
    }
  };

  const first = page.total === 0 ? 0 : offset + 1;
  const last = Math.min(offset + PAGE_SIZE, page.total);

  return (
    <section className="import-screen">
      <div className="page-heading">
        <p className="eyebrow">Transaction manager</p>
        <h1>Review imported activity</h1>
        <p>Search and edits stay local. Imported narration is preserved even when a merchant name is corrected.</p>
      </div>

      <form
        className="panel filter-bar"
        onSubmit={(event) => {
          event.preventDefault();
          setFilter({ search: searchText });
        }}
      >
        <label className="grow">
          Search
          <input value={searchText} onChange={(event) => setSearchText(event.target.value)} placeholder="Narration or merchant" />
        </label>
        <label>
          From
          <input type="date" value={filters.dateFrom ?? ""} onChange={(event) => setFilter({ dateFrom: event.target.value })} />
        </label>
        <label>
          To
          <input type="date" value={filters.dateTo ?? ""} onChange={(event) => setFilter({ dateTo: event.target.value })} />
        </label>
        <label>
          Account
          <select value={filters.accountId ?? ""} onChange={(event) => setFilter({ accountId: event.target.value })}>
            <option value="">All accounts</option>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>{account.name}</option>
            ))}
          </select>
        </label>
        <label>
          Direction
          <select value={filters.direction ?? ""} onChange={(event) => setFilter({ direction: event.target.value })}>
            <option value="">Any</option>
            <option value="debit">Money out</option>
            <option value="credit">Money in</option>
          </select>
        </label>
        <label>
          Category
          <select value={filters.categoryId ?? ""} onChange={(event) => setFilter({ categoryId: event.target.value })}>
            <option value="">All categories</option>
            {categories.map((category) => (
              <option key={category.id} value={category.id}>{category.name}</option>
            ))}
          </select>
        </label>
        <label>
          <input
            type="checkbox"
            checked={reviewOnly}
            onChange={(event) => {
              setOffset(0);
              setReviewOnly(event.target.checked);
            }}
          />
          Needs review
        </label>
        <button type="submit" className="secondary">Search</button>
        {hasFilters && <button type="button" className="secondary" onClick={clearFilters}>Clear filters</button>}
      </form>

      <div className="panel">
        <strong>{loading ? "Loading transactions…" : `${page.total} matching transactions`}</strong>
        {!loading && page.items.length === 0 ? (
          <p>{hasFilters ? "No transactions match these filters." : "No transactions yet. Import a statement to see them here."}</p>
        ) : (
          <div className="transaction-list" aria-busy={loading}>
            {page.items.map((item) => (
              <button
                type="button"
                className={item.id === selected?.id ? "transaction-row selected" : "transaction-row"}
                key={item.id}
                onClick={() => setSelected(item)}
              >
                <span>{item.transactionDate}</span>
                <span>
                  {!item.isReviewed && <i className="review-dot" aria-label="Needs review" />}
                  {item.merchantOverride || item.rawDescription}
                </span>
                <span>{item.categoryName ?? (item.transactionKind === "unknown" ? "Unclassified" : item.transactionKind)}</span>
                <strong className={item.direction === "debit" ? "debit" : "credit"}>
                  {item.direction === "debit" ? "−" : "+"}
                  {currency.format(item.amountPaise / 100)}
                </strong>
              </button>
            ))}
          </div>
        )}
        <div className="commit-row">
          <span>{page.total > 0 ? `${first}–${last} of ${page.total}` : ""}</span>
          <span className="button-group">
            <button type="button" className="secondary" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>Previous</button>
            <button type="button" className="secondary" disabled={offset + PAGE_SIZE >= page.total} onClick={() => setOffset(offset + PAGE_SIZE)}>Next</button>
          </span>
        </div>
      </div>

      {selected && (
        <div className="panel">
          <h2>Review transaction</h2>
          <dl className="detail-list">
            <div><dt>Imported as</dt><dd>{selected.rawDescription}</dd></div>
            <div><dt>Account</dt><dd>{selected.accountName}</dd></div>
            <div><dt>Kind</dt><dd>{selected.transactionKind}</dd></div>
            <div>
              <dt>Source</dt>
              <dd>
                {selected.sourceFilename
                  ? `${selected.sourceFilename}${selected.sourceRowNumber ? `, row ${selected.sourceRowNumber}` : ""}${selected.importedAt ? ` · imported ${selected.importedAt.slice(0, 10)}` : ""}`
                  : "Not linked to an import"}
              </dd>
            </div>
          </dl>
          <div className="inline-form">
            <label>
              Merchant
              <input
                value={selected.merchantOverride ?? ""}
                onChange={(event) => setSelected({ ...selected, merchantOverride: event.target.value })}
                placeholder="Corrected merchant"
              />
            </label>
            <label>
              Category
              <select
                value={selected.categoryId ?? ""}
                onChange={(event) => setSelected({ ...selected, categoryId: event.target.value || null })}
              >
                <option value="">No category</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>{category.name}</option>
                ))}
              </select>
            </label>
            <label>
              <input
                type="checkbox"
                checked={selected.isReviewed}
                onChange={(event) => setSelected({ ...selected, isReviewed: event.target.checked })}
              />
              Reviewed
            </label>
            <button type="button" onClick={() => void save()}>Save review</button>
            <button type="button" className="secondary" onClick={() => setSelected(null)}>Close</button>
          </div>
        </div>
      )}
    </section>
  );
}
