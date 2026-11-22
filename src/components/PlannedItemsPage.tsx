import { useCallback, useEffect, useState } from "react";
import { AccountWithBalance, api, Category, errorMessage, NewPlannedItem, PlannedItem, plannedApi, Schedule } from "../api";
import { categoryChoices } from "../categories";
import { formatMinor, parseAmount, toAmountInput } from "../money";
import { localToday } from "../periods";
import { lessOftenThanMonthly, scheduleLabel, signedAmount } from "../planned";

type Editing = { item: PlannedItem | null } | null;

/** Planned items: Income or Expenses expected on a schedule. */
export default function PlannedItemsPage({ currency }: { currency: string }) {
  const [items, setItems] = useState<PlannedItem[] | null>(null);
  const [accounts, setAccounts] = useState<AccountWithBalance[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [editing, setEditing] = useState<Editing>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    Promise.all([plannedApi.list(), api.accountsList(), api.categoriesList()]).then(([i, a, c]) => {
      setItems(i);
      setAccounts(a);
      setCategories(c);
    }, (e) => setError(errorMessage(e)));
  }, []);
  useEffect(reload, [reload]);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      reload();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    }
  }

  const today = localToday();
  const accountName = (id: string) => accounts.find((a) => a.id === id)?.name ?? "Deleted Account";
  const categoryName = (id: string) => categories.find((c) => c.id === id)?.name ?? "Deleted Category";
  const money = (minor: number) => (
    <span className={minor < 0 ? "amount negative" : "amount"}>{formatMinor(minor, currency)}</span>
  );

  function end(item: PlannedItem) {
    if (window.confirm(`End "${item.description}"? Today is the last day it can occur.`)) {
      run(() => plannedApi.end(item.id, today));
    }
  }

  function remove(item: PlannedItem) {
    if (window.confirm(`Delete "${item.description}"? To keep its past occurrences, end it instead.`)) {
      run(() => plannedApi.delete(item.id));
    }
  }

  const canAdd = accounts.length > 0 && categories.length > 0;

  return (
    <>
      <div className="heading">
        <h1>Planned items</h1>
        {editing === null && (
          <button className="primary" onClick={() => setEditing({ item: null })} disabled={!canAdd}>
            Add Planned item
          </button>
        )}
      </div>
      <p className="muted small">
        Income or Expenses you expect on a schedule, such as salary every month or insurance once a year. Their expected
        occurrences count in Spendable. Items that occur less often than monthly are spread as Set-asides: their yearly
        total divided by 12 is set aside from every Period instead.
      </p>
      {error && <p className="error">{error}</p>}

      {editing && (
        <PlannedItemForm
          currency={currency}
          accounts={accounts}
          categories={categories}
          item={editing.item}
          onSubmit={async (fields) => {
            const ok = await run(() =>
              editing.item
                ? plannedApi.update({ ...fields, id: editing.item.id, set_aside: fields.set_aside ?? false })
                : plannedApi.create(fields),
            );
            if (ok) setEditing(null);
          }}
          onCancel={() => setEditing(null)}
        />
      )}

      {items && items.length === 0 && !editing && (
        <p className="muted">
          {canAdd ? "No Planned items yet." : "Add an Account and a Category first, then you can plan Income and Expenses."}
        </p>
      )}

      {items && items.length > 0 && (
        <ul className="list">
          {items.map((item) => {
            const ended = item.end_date !== null && item.end_date < today;
            return (
              <li key={item.id} className="row-item">
                <span className="grow">
                  <span className="name">{item.description}</span>{" "}
                  {item.set_aside && <span className="badge muted-badge">Set-aside</span>}{" "}
                  {ended && <span className="badge muted-badge">Ended {item.end_date}</span>}
                  <br />
                  <span className="muted small">
                    {scheduleLabel(item.schedule)} · {categoryName(item.category_id)} · {accountName(item.account_id)} ·
                    from {item.start_date}
                    {item.end_date && !ended ? ` to ${item.end_date}` : ""}
                  </span>
                </span>
                {money(item.amount_minor)}
                <span className="actions">
                  <button onClick={() => setEditing({ item })}>Edit</button>
                  {!ended && <button onClick={() => end(item)}>End</button>}
                  <button className="danger" onClick={() => remove(item)}>
                    Delete
                  </button>
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </>
  );
}

type Repeat = "monthly" | "every_n_months" | "yearly";

type FormProps = {
  currency: string;
  accounts: AccountWithBalance[];
  categories: Category[];
  item: PlannedItem | null;
  onSubmit: (fields: NewPlannedItem) => void;
  onCancel: () => void;
};

function PlannedItemForm({ currency, accounts, categories, item, onSubmit, onCancel }: FormProps) {
  const groups = categoryChoices(categories);
  const known = item !== null && categories.some((c) => c.id === item.category_id);
  const s = item?.schedule;
  const [accountId, setAccountId] = useState(item?.account_id ?? accounts[0]?.id ?? "");
  const [categoryId, setCategoryId] = useState(known ? item.category_id : (groups[0]?.options[0]?.id ?? ""));
  const [description, setDescription] = useState(item?.description ?? "");
  const [amount, setAmount] = useState(item ? toAmountInput(Math.abs(item.amount_minor), currency) : "");
  const [repeat, setRepeat] = useState<Repeat>(
    s?.kind === "yearly" ? "yearly" : s && s.every_months > 1 ? "every_n_months" : "monthly",
  );
  const [day, setDay] = useState(String(s?.day ?? 1));
  const [everyMonths, setEveryMonths] = useState(String(s?.kind === "monthly" && s.every_months > 1 ? s.every_months : 3));
  const [month, setMonth] = useState(String(s?.kind === "yearly" ? s.month : 1));
  const [startDate, setStartDate] = useState(item?.start_date ?? localToday());
  const [endDate, setEndDate] = useState(item?.end_date ?? "");
  // null = follow the schedule's default until the user ticks or unticks it.
  const [setAside, setSetAside] = useState<boolean | null>(item ? item.set_aside : null);
  const [error, setError] = useState<string | null>(null);

  const schedule: Schedule =
    repeat === "yearly"
      ? { kind: "yearly", month: Number(month), day: Number(day) }
      : { kind: "monthly", day: Number(day), every_months: repeat === "monthly" ? 1 : Number(everyMonths) };
  const spreadable = lessOftenThanMonthly(schedule);
  const spread = spreadable && (setAside ?? true);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const parsed = parseAmount(amount, currency);
    if (!parsed.ok) {
      setError(`Amount: ${parsed.error}`);
      return;
    }
    const kind = categories.find((c) => c.id === categoryId)?.kind ?? "expense";
    setError(null);
    onSubmit({
      account_id: accountId,
      category_id: categoryId,
      description,
      amount_minor: signedAmount(kind, parsed.minor),
      schedule,
      start_date: startDate,
      end_date: endDate || null,
      set_aside: spreadable ? (item || setAside !== null ? spread : null) : false,
    });
  }

  const monthNames = Array.from({ length: 12 }, (_, i) =>
    new Intl.DateTimeFormat(undefined, { month: "long" }).format(new Date(2024, i, 1)),
  );

  return (
    <form className="card form" onSubmit={submit}>
      <h2>{item ? "Edit Planned item" : "New Planned item"}</h2>
      <div className="row">
        <label className="field">
          <span>Description</span>
          <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Salary" autoFocus />
        </label>
        <label className="field">
          <span>Amount ({currency})</span>
          <input value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" placeholder="600.00" />
        </label>
      </div>
      <div className="row">
        <label className="field">
          <span>Category (decides Income or Expense)</span>
          <select value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
            {!known && item && <option value={item.category_id}>Deleted Category (pick another)</option>}
            {groups.map((g) => (
              <optgroup key={g.kind} label={g.label}>
                {g.options.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.label}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Account</span>
          <select value={accountId} onChange={(e) => setAccountId(e.target.value)}>
            {item && !accounts.some((a) => a.id === item.account_id) && (
              <option value={item.account_id}>Deleted Account (pick another)</option>
            )}
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="row">
        <label className="field">
          <span>Repeats</span>
          <select value={repeat} onChange={(e) => setRepeat(e.target.value as Repeat)}>
            <option value="monthly">Monthly on a day</option>
            <option value="every_n_months">Every N months</option>
            <option value="yearly">Yearly on a date</option>
          </select>
        </label>
        {repeat === "every_n_months" && (
          <label className="field">
            <span>Every (months)</span>
            <input type="number" min={2} max={120} value={everyMonths} onChange={(e) => setEveryMonths(e.target.value)} />
          </label>
        )}
        {repeat === "yearly" && (
          <label className="field">
            <span>Month</span>
            <select value={month} onChange={(e) => setMonth(e.target.value)}>
              {monthNames.map((name, i) => (
                <option key={name} value={i + 1}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="field">
          <span>Day</span>
          <input type="number" min={1} max={31} value={day} onChange={(e) => setDay(e.target.value)} />
        </label>
      </div>
      <div className="row">
        <label className="field">
          <span>Starts on</span>
          <input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} />
        </label>
        <label className="field">
          <span>Ends on (optional)</span>
          <input type="date" value={endDate} onChange={(e) => setEndDate(e.target.value)} />
        </label>
      </div>
      {repeat === "every_n_months" && (
        <p className="muted small">Counted from the month of the start date.</p>
      )}
      {spreadable && (
        <label className="check">
          <input type="checkbox" checked={spread} onChange={(e) => setSetAside(e.target.checked)} />
          Spread as Set-asides: set aside its yearly total divided by 12 from every Period, instead of counting it in the
          Period it falls due
        </label>
      )}
      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button type="submit" className="primary">
          {item ? "Save" : "Add Planned item"}
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
