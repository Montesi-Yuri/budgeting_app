import { useCallback, useEffect, useState } from "react";
import { AccountWithBalance, api, Category, errorMessage, Transaction, TransactionView } from "../api";
import { FLOW_LABEL, transactionFlow } from "../categories";
import { formatMinor } from "../money";
import TransactionForm from "./TransactionForm";

/** The Transaction list of one Account, newest first, with add/edit/delete. */
export default function TransactionsPage({ currency }: { currency: string }) {
  const [accounts, setAccounts] = useState<AccountWithBalance[] | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [transactions, setTransactions] = useState<TransactionView[] | null>(null);
  const [editing, setEditing] = useState<Transaction | "new" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.accountsList(), api.categoriesList()]).then(([accs, cats]) => {
      setAccounts(accs);
      setCategories(cats);
      setAccountId((current) => current ?? accs[0]?.id ?? null);
    }, (e) => setError(errorMessage(e)));
  }, []);

  const reload = useCallback(() => {
    if (!accountId) return;
    api.accountsList().then(setAccounts, (e) => setError(errorMessage(e)));
    api.transactionsList(accountId).then(setTransactions, (e) => setError(errorMessage(e)));
  }, [accountId]);
  useEffect(reload, [reload]);

  async function remove(t: Transaction) {
    if (!window.confirm(`Delete the Transaction "${t.description}"?`)) return;
    try {
      await api.transactionDelete(t.id);
      reload();
    } catch (e) {
      setError(errorMessage(e));
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
        {editing === null && accountId && (
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
            <select value={accountId ?? ""} onChange={(e) => setAccountId(e.target.value)}>
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

      {editing !== null && accounts && accountId && (
        <TransactionForm
          currency={currency}
          accounts={accounts}
          categories={categories}
          accountId={accountId}
          transaction={editing === "new" ? null : editing}
          onDone={() => {
            setEditing(null);
            reload();
          }}
          onCancel={() => setEditing(null)}
        />
      )}

      {transactions && transactions.length === 0 && <p className="muted">No Transactions on this Account yet.</p>}
      {transactions && transactions.length > 0 && (
        <ul className="list">
          {transactions.map((t) => (
            <li key={t.id} className="row-item">
              <TransactionSummary t={t} currency={currency} />
              <span className="actions">
                <button onClick={() => setEditing(t)}>Edit</button>
                <button className="danger" onClick={() => remove(t)}>
                  Delete
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
      {account && (
        <p className="muted small">
          The balance is the Opening balance on {account.opening_date} plus every Transaction after that day.
        </p>
      )}
    </>
  );
}

export function Money({ minor, currency }: { minor: number; currency: string }) {
  return <span className={minor < 0 ? "amount negative" : "amount"}>{formatMinor(minor, currency)}</span>;
}

/** Date, description, Category (or Uncategorised) and amount of a Transaction. */
export function TransactionSummary({ t, currency, showAccount }: { t: TransactionView; currency: string; showAccount?: boolean }) {
  const flow = transactionFlow(t.category_kind, t.amount_minor);
  return (
    <>
      <span className="date muted small">{t.date}</span>
      <span className="grow">
        <span className="name">{t.description}</span>
        <span className="muted small">
          {" · "}
          {t.category_name ?? "Uncategorised"}
          {flow === "refund" && <span className="badge refund">{FLOW_LABEL.refund}</span>}
          {showAccount && ` · ${t.account_name}`}
          {t.note && ` · ${t.note}`}
        </span>
      </span>
      <Money minor={t.amount_minor} currency={currency} />
    </>
  );
}
