import { useCallback, useEffect, useMemo, useState } from "react";
import { AccountWithBalance, Category, errorMessage, Transaction, TransactionView, accountsApi, categoriesApi, transactionsApi } from "@/shared/api";
import { Money } from "@/shared/ui";
import { localToday, periodLabel, periodOf, shiftPeriod } from "@/shared/lib/periods";
import { categoryChoices, FLOW_LABEL, transactionFlow } from "@/features/categories";
import { CategorySelect } from "@/features/categories";
import {
  ALL_TRANSACTIONS,
  CategoryFilter,
  filterTransactions,
  isFiltered,
  NEWEST_FIRST,
  SortKey,
  sortTransactions,
  TransactionFilter,
  TransactionSort,
} from "../lib/transactionTable";
import TransactionDialog from "./TransactionDialog";

// The filters and sort outlive the page (switching tab or Account) until the app restarts.
let remembered: { filter: TransactionFilter; sort: TransactionSort } = { filter: ALL_TRANSACTIONS, sort: NEWEST_FIRST };

/** The Transaction table of one Account, with sorting, filters and add/edit/delete. */
export default function TransactionsPage({ currency }: { currency: string }) {
  const [accounts, setAccounts] = useState<AccountWithBalance[] | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [transactions, setTransactions] = useState<TransactionView[] | null>(null);
  const [editing, setEditing] = useState<Transaction | "new" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilterState] = useState(remembered.filter);
  const [sort, setSortState] = useState(remembered.sort);
  // Rows whose Category was changed in the table stay visible until the filters or the Account change.
  const [kept, setKept] = useState<Set<string>>(new Set());
  // Category changes being saved, shown in the row until the list reloads.
  const [saving, setSaving] = useState<Map<string, string | null>>(new Map());

  function setFilter(next: TransactionFilter) {
    remembered = { ...remembered, filter: next };
    setFilterState(next);
    setKept(new Set());
  }
  function setSort(next: TransactionSort) {
    remembered = { ...remembered, sort: next };
    setSortState(next);
  }

  useEffect(() => {
    Promise.all([accountsApi.list(), categoriesApi.list()]).then(([accs, cats]) => {
      setAccounts(accs);
      setCategories(cats);
      setAccountId((current) => current ?? accs[0]?.id ?? null);
    }, (e) => setError(errorMessage(e)));
  }, []);

  const reload = useCallback(() => {
    if (!accountId) return;
    accountsApi.list().then(setAccounts, (e) => setError(errorMessage(e)));
    return transactionsApi.list(accountId).then(setTransactions, (e) => setError(errorMessage(e)));
  }, [accountId]);
  useEffect(() => {
    void reload();
  }, [reload]);

  // A remembered Category may have been deleted since (e.g. on another Device): show everything instead.
  const category = filter.category;
  const activeFilter: TransactionFilter =
    typeof category === "object" && !categories.some((c) => c.id === category.categoryId)
      ? { ...filter, category: "all" }
      : filter;

  const shown = useMemo(() => {
    if (!transactions) return null;
    const matching = new Set(filterTransactions(transactions, activeFilter, categories).map((t) => t.id));
    return sortTransactions(
      transactions.filter((t) => matching.has(t.id) || kept.has(t.id)),
      sort,
    );
  }, [transactions, activeFilter, categories, kept, sort]);

  /** Called from the Edit dialog, which shows the error if it fails. */
  async function remove(t: Transaction) {
    if (!window.confirm(`Delete the Transaction "${t.description}"?`)) return;
    await transactionsApi.delete(t.id);
    setEditing(null);
    void reload();
  }

  async function setCategory(t: TransactionView, categoryId: string | null) {
    setError(null);
    setSaving((m) => new Map(m).set(t.id, categoryId));
    setKept((k) => new Set(k).add(t.id));
    try {
      await transactionsApi.setCategory(t.id, categoryId);
      await reload();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving((m) => {
        const next = new Map(m);
        next.delete(t.id);
        return next;
      });
    }
  }

  if (accounts && accounts.length === 0) {
    return (
      <>
        <h1>Transactions</h1>
        <p className="muted">Add an Account first.</p>
      </>
    );
  }
  const account = accounts?.find((a) => a.id === accountId);

  return (
    <>
      <div className="heading">
        <h1>Transactions</h1>
        {accountId && (
          <button className="primary" onClick={() => setEditing("new")}>
            Add Transaction
          </button>
        )}
      </div>
      {error && <p className="error">{error}</p>}
      {accounts && (
        <div className="row toolbar">
          <label className="field">
            <span>Account</span>
            <select
              value={accountId ?? ""}
              onChange={(e) => {
                setAccountId(e.target.value);
                setKept(new Set());
              }}
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          {account && (
            <p className="net">
              Balance <Money minor={account.balance_minor} currency={currency} />
            </p>
          )}
        </div>
      )}

      <Filters filter={activeFilter} categories={categories} onChange={setFilter} />

      {editing !== null && accounts && accountId && (
        <TransactionDialog
          currency={currency}
          accounts={accounts}
          categories={categories}
          accountId={accountId}
          transaction={editing === "new" ? null : editing}
          onDone={() => {
            setEditing(null);
            void reload();
          }}
          onCancel={() => setEditing(null)}
          onDelete={editing === "new" ? undefined : () => remove(editing)}
        />
      )}

      {transactions && transactions.length > 0 && shown && isFiltered(activeFilter) && (
        <p className="muted small">
          {shown.length} of {transactions.length} Transactions · total{" "}
          <Money minor={shown.reduce((sum, t) => sum + t.amount_minor, 0)} currency={currency} />
        </p>
      )}
      {transactions && transactions.length === 0 && <p className="muted">No Transactions on this Account yet.</p>}
      {transactions && transactions.length > 0 && shown?.length === 0 && (
        <p className="muted">
          No Transactions match these filters.{" "}
          <button onClick={() => setFilter(ALL_TRANSACTIONS)}>Clear filters</button>
        </p>
      )}
      {shown && shown.length > 0 && (
        <div className="table-wrap">
          <table className="transactions">
            <thead>
              <tr>
                <SortHeader label="Date" sortKey="date" sort={sort} onSort={setSort} />
                <SortHeader label="Description" sortKey="description" sort={sort} onSort={setSort} />
                <SortHeader label="Category" sortKey="category" sort={sort} onSort={setSort} className="col-category" />
                <th className="col-note">Note</th>
                <SortHeader label="Amount" sortKey="amount" sort={sort} onSort={setSort} className="num" />
              </tr>
            </thead>
            <tbody>
              {shown.map((t) => {
                const refund = transactionFlow(t.category_kind, t.amount_minor) === "refund";
                const pending = saving.has(t.id);
                return (
                  <tr
                    key={t.id}
                    tabIndex={0}
                    aria-label={`Edit ${t.description}`}
                    onClick={() => setEditing(t)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && e.target === e.currentTarget) setEditing(t);
                    }}
                  >
                    <td className="date">{t.date}</td>
                    <td>
                      <span className="name">{t.description}</span>
                      <span className="sub muted small">
                        {t.category_name ?? "Uncategorised"}
                        {refund && <span className="badge refund">{FLOW_LABEL.refund}</span>}
                      </span>
                    </td>
                    <td className="col-category">
                      {/* The picker edits in place; the rest of the row opens the dialog. */}
                      <span onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                        <CategorySelect
                          aria-label={`Category for ${t.description}`}
                          categories={categories}
                          value={pending ? (saving.get(t.id) ?? null) : t.category_id}
                          disabled={pending}
                          onChange={(categoryId) => setCategory(t, categoryId)}
                        />
                      </span>
                      {refund && <span className="badge refund">{FLOW_LABEL.refund}</span>}
                    </td>
                    <td className="col-note muted small">{t.note}</td>
                    <td className="num">
                      <Money minor={t.amount_minor} currency={currency} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      {account && (
        <p className="muted small">
          The balance is the Opening balance on {account.opening_date} plus every Transaction after that day.
        </p>
      )}
    </>
  );
}

/** The Category filter <select> value for Uncategorised. */
const UNCATEGORISED_VALUE = "uncategorised";

function Filters({
  filter,
  categories,
  onChange,
}: {
  filter: TransactionFilter;
  categories: Category[];
  onChange: (filter: TransactionFilter) => void;
}) {
  const categoryValue =
    filter.category === "all" ? "" : filter.category === "uncategorised" ? UNCATEGORISED_VALUE : filter.category.categoryId;
  const toCategory = (value: string): CategoryFilter =>
    value === "" ? "all" : value === UNCATEGORISED_VALUE ? "uncategorised" : { categoryId: value };
  const stepPeriod = (delta: number) =>
    onChange({ ...filter, period: filter.period === null ? periodOf(localToday()) : shiftPeriod(filter.period, delta) });

  return (
    <div className="row toolbar filters">
      <label className="field">
        <span>Search</span>
        <input
          type="search"
          value={filter.text}
          placeholder="Description or note"
          onChange={(e) => onChange({ ...filter, text: e.target.value })}
        />
      </label>
      <label className="field">
        <span>Category</span>
        <select value={categoryValue} onChange={(e) => onChange({ ...filter, category: toCategory(e.target.value) })}>
          <option value="">All Categories</option>
          <option value={UNCATEGORISED_VALUE}>Uncategorised</option>
          {categoryChoices(categories).map((group) => (
            <optgroup key={group.kind} label={group.label}>
              {group.options.map((o) => (
                <option key={o.id} value={o.id}>
                  {o.label}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </label>
      <div className="field">
        <span>Period</span>
        <div className="period-step">
          <button aria-label="Previous Period" onClick={() => stepPeriod(-1)}>
            ‹
          </button>
          <span className="period-label">{filter.period === null ? "All Periods" : periodLabel(filter.period)}</span>
          <button aria-label="Next Period" onClick={() => stepPeriod(1)}>
            ›
          </button>
          {filter.period !== null && (
            <button className="link" onClick={() => onChange({ ...filter, period: null })}>
              All
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function SortHeader({
  label,
  sortKey,
  sort,
  onSort,
  className,
}: {
  label: string;
  sortKey: SortKey;
  sort: TransactionSort;
  onSort: (sort: TransactionSort) => void;
  className?: string;
}) {
  const active = sort.key === sortKey;
  // A new column starts with the order you most likely want: newest, A–Z, biggest spend first.
  const first = sortKey === "date" ? "desc" : "asc";
  return (
    <th className={className} aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}>
      <button
        className="sort"
        onClick={() => onSort({ key: sortKey, dir: active ? (sort.dir === "asc" ? "desc" : "asc") : first })}
      >
        {label}
        {active && <span aria-hidden="true">{sort.dir === "asc" ? " ▲" : " ▼"}</span>}
      </button>
    </th>
  );
}
