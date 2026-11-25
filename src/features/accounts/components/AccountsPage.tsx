import { useCallback, useEffect, useState } from "react";
import { Account, AccountWithBalance, errorMessage, accountsApi } from "@/shared/api";
import { formatMinor } from "@/shared/lib/money";
import AccountForm from "./AccountForm";
import { ACCOUNT_KINDS } from "../lib/accountKinds";

const kindLabel = (kind: Account["kind"]) => ACCOUNT_KINDS.find((k) => k.kind === kind)?.label ?? kind;

export default function AccountsPage({ currency }: { currency: string }) {
  const [accounts, setAccounts] = useState<AccountWithBalance[] | null>(null);
  const [editing, setEditing] = useState<Account | "new" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    accountsApi.list().then(setAccounts, (e) => setError(errorMessage(e)));
  }, []);
  useEffect(reload, [reload]);

  async function remove(account: Account) {
    if (!window.confirm(`Delete the Account "${account.name}"?`)) return;
    try {
      await accountsApi.delete(account.id);
      reload();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const netPosition = accounts?.reduce((sum, a) => sum + a.balance_minor, 0) ?? 0;
  const money = (minor: number) => (
    <span className={minor < 0 ? "amount negative" : "amount"}>{formatMinor(minor, currency)}</span>
  );

  return (
    <>
      <div className="heading">
        <h1>Accounts</h1>
        {editing === null && (
          <button className="primary" onClick={() => setEditing("new")}>
            Add Account
          </button>
        )}
      </div>
      {error && <p className="error">{error}</p>}

      {editing !== null && (
        <AccountForm
          currency={currency}
          account={editing === "new" ? null : editing}
          onDone={() => {
            setEditing(null);
            reload();
          }}
          onCancel={() => setEditing(null)}
        />
      )}

      {accounts && accounts.length === 0 && editing === null && (
        <p className="muted">No Accounts yet. Add your current account, savings accounts and credit cards.</p>
      )}

      {accounts && accounts.length > 0 && (
        <>
          <p className="net">Net position {money(netPosition)}</p>
          <ul className="accounts">
            {accounts.map((a) => (
              <li key={a.id} className="card account">
                <div>
                  <div className="name">
                    {a.name} {a.is_main && <span className="badge">Main</span>}
                  </div>
                  <div className="muted small">
                    {kindLabel(a.kind)} · Opening balance {formatMinor(a.opening_minor, currency)} on{" "}
                    {a.opening_date}
                  </div>
                </div>
                <div className="balance">{money(a.balance_minor)}</div>
                <div className="actions">
                  <button onClick={() => setEditing(a)}>Edit</button>
                  <button className="danger" onClick={() => remove(a)}>
                    Delete
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
    </>
  );
}
