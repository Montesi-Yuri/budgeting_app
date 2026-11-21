import { useState } from "react";
import { ACCOUNT_KINDS, Account, AccountKind, api, errorMessage } from "../api";
import { parseAmount, toAmountInput } from "../money";

type Props = {
  currency: string;
  /** null = create a new Account */
  account: Account | null;
  onDone: () => void;
  onCancel: () => void;
};

const today = () => new Date().toLocaleDateString("sv-SE"); // YYYY-MM-DD in local time

export default function AccountForm({ currency, account, onDone, onCancel }: Props) {
  const [name, setName] = useState(account?.name ?? "");
  const [kind, setKind] = useState<AccountKind>(account?.kind ?? "current");
  const [opening, setOpening] = useState(account ? toAmountInput(account.opening_minor, currency) : "");
  const [openingDate, setOpeningDate] = useState(account?.opening_date ?? today());
  const [isMain, setIsMain] = useState(account?.is_main ?? false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const amount = parseAmount(opening, currency);
    if (!amount.ok) {
      setError(`Opening balance: ${amount.error}`);
      return;
    }
    const fields = { name, kind, opening_minor: amount.minor, opening_date: openingDate, is_main: isMain };
    setBusy(true);
    setError(null);
    try {
      if (account) await api.accountUpdate({ ...fields, id: account.id });
      else await api.accountCreate(fields);
      onDone();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="card form" onSubmit={submit}>
      <h2>{account ? "Edit Account" : "New Account"}</h2>
      <label className="field">
        <span>Name</span>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Everyday account" autoFocus />
      </label>
      <label className="field">
        <span>Kind</span>
        <select value={kind} onChange={(e) => setKind(e.target.value as AccountKind)}>
          {ACCOUNT_KINDS.map((k) => (
            <option key={k.kind} value={k.kind}>
              {k.label}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <label className="field">
          <span>Opening balance ({currency})</span>
          <input value={opening} onChange={(e) => setOpening(e.target.value)} inputMode="decimal" placeholder="0.00" />
        </label>
        <label className="field">
          <span>on</span>
          <input type="date" value={openingDate} onChange={(e) => setOpeningDate(e.target.value)} />
        </label>
      </div>
      {kind === "credit_card" && (
        <p className="muted small">Money owed on a credit card is negative: enter a debt of 350 as -350.</p>
      )}
      <label className="check">
        <input type="checkbox" checked={isMain} onChange={(e) => setIsMain(e.target.checked)} />
        Main Account (the current account shown in the Forecast)
      </label>
      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button type="submit" className="primary" disabled={busy}>
          {account ? "Save" : "Add Account"}
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
