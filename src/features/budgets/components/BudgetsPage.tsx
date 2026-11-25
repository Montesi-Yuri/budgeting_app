import { useCallback, useEffect, useState } from "react";
import { Budget, BudgetPer, BudgetProgress, Category, errorMessage, budgetsApi, categoriesApi } from "@/shared/api";
import { formatMinor, parseAmount, toAmountInput } from "@/shared/lib/money";
import { useAction } from "@/shared/hooks";
import { categoryChoices } from "@/features/categories";
import { budgetMeter } from "../lib/budgets";

const today = () => new Date().toLocaleDateString("sv-SE"); // YYYY-MM-DD in local time

const PER_LABEL: Record<BudgetPer, string> = { period: "per Period", week: "per Week" };

type Editing = { budget: Budget | null } | null;

export default function BudgetsPage({ currency }: { currency: string }) {
  const [progress, setProgress] = useState<BudgetProgress[] | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [editing, setEditing] = useState<Editing>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    Promise.all([budgetsApi.progress(today()), categoriesApi.list()]).then(([p, c]) => {
      setProgress(p);
      setCategories(c);
    }, (e) => setError(errorMessage(e)));
  }, []);
  useEffect(reload, [reload]);

  const run = useAction(reload, setError);

  const expenseOptions = categoryChoices(categories).find((g) => g.kind === "expense")?.options ?? [];
  const nameOf = (id: string) => expenseOptions.find((o) => o.id === id)?.label ?? "Deleted Category";
  const money = (minor: number) => formatMinor(minor, currency);

  function end(b: Budget) {
    if (window.confirm(`End the Budget for "${nameOf(b.category_id)}"? Today is the last day it applies.`)) {
      run(() => budgetsApi.end(b.id, today()));
    }
  }

  function remove(b: Budget) {
    if (window.confirm(`Delete the Budget for "${nameOf(b.category_id)}"?`)) run(() => budgetsApi.delete(b.id));
  }

  return (
    <>
      <div className="heading">
        <h1>Budgets</h1>
        {editing === null && (
          <button className="primary" onClick={() => setEditing({ budget: null })} disabled={expenseOptions.length === 0}>
            Add Budget
          </button>
        )}
      </div>
      <p className="muted small">
        A Budget is an optional spending limit for an expense Category, per Period or per Week. A Budget on a parent
        Category covers its children. It only warns when a Category goes over: it doesn't reserve money, and Spendable
        still covers all spending.
      </p>
      {error && <p className="error">{error}</p>}

      {editing && (
        <BudgetForm
          currency={currency}
          options={expenseOptions}
          budget={editing.budget}
          onSubmit={async (fields) => {
            const ok = await run(() =>
              editing.budget ? budgetsApi.update({ ...fields, id: editing.budget.id }) : budgetsApi.create(fields),
            );
            if (ok) setEditing(null);
          }}
          onCancel={() => setEditing(null)}
        />
      )}

      {progress && progress.length === 0 && !editing && (
        <p className="muted">
          {expenseOptions.length === 0
            ? "Add an expense Category first, then you can give it a Budget."
            : "No Budgets yet. Add one to see how much of a Category's limit is left this Period or Week."}
        </p>
      )}

      {progress?.map((p) => {
        const b = p.budget;
        const meter = budgetMeter(p.spent_minor, b.amount_minor);
        return (
          <section key={b.id} className={p.over ? "card budget over" : "card budget"}>
            <div className="budget-head">
              <span className="name">
                {nameOf(b.category_id)}{" "}
                {p.state === "broken" && <span className="badge broken">Broken: Category deleted</span>}
                {p.over && <span className="badge over">Over by {money(-p.left_minor)}</span>}
                {p.state === "upcoming" && <span className="badge muted-badge">Starts {b.valid_from}</span>}
                {p.state === "ended" && <span className="badge muted-badge">Ended {b.valid_to}</span>}
              </span>
              <span className="muted small">
                {money(b.amount_minor)} {PER_LABEL[b.per]}
              </span>
            </div>
            {p.state === "active" && p.range && (
              <>
                <div className={`meter ${meter.tone}`} role="progressbar" aria-valuenow={meter.percent} aria-valuemin={0} aria-valuemax={100}>
                  <span style={{ width: `${meter.percent}%` }} />
                </div>
                <div className="budget-head small">
                  <span>
                    Spent <span className="amount">{money(p.spent_minor)}</span>
                  </span>
                  <span className={p.left_minor < 0 ? "amount negative" : "amount"}>
                    {p.left_minor < 0 ? `${money(-p.left_minor)} over` : `${money(p.left_minor)} left`}
                  </span>
                </div>
                <span className="muted small">
                  {b.per === "week" ? "This Week" : "This Period"}: {p.range.from} to {p.range.to}
                </span>
              </>
            )}
            {p.state === "broken" && (
              <span className="muted small">This Budget is not applied. Edit it to pick another Category.</span>
            )}
            <div className="actions">
              <button onClick={() => setEditing({ budget: b })}>Edit</button>
              {p.state !== "ended" && p.state !== "upcoming" && <button onClick={() => end(b)}>End</button>}
              <button className="danger" onClick={() => remove(b)}>
                Delete
              </button>
            </div>
          </section>
        );
      })}
    </>
  );
}

type FormProps = {
  currency: string;
  options: { id: string; label: string }[];
  budget: Budget | null;
  onSubmit: (fields: { category_id: string; per: BudgetPer; amount_minor: number; valid_from: string }) => void;
  onCancel: () => void;
};

function BudgetForm({ currency, options, budget, onSubmit, onCancel }: FormProps) {
  const known = budget !== null && options.some((o) => o.id === budget.category_id);
  const [categoryId, setCategoryId] = useState(known ? budget.category_id : (options[0]?.id ?? ""));
  const [per, setPer] = useState<BudgetPer>(budget?.per ?? "period");
  const [amount, setAmount] = useState(budget ? toAmountInput(budget.amount_minor, currency) : "");
  const [validFrom, setValidFrom] = useState(budget?.valid_from ?? today());
  const [error, setError] = useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const parsed = parseAmount(amount, currency);
    if (!parsed.ok) {
      setError(`Amount: ${parsed.error}`);
      return;
    }
    setError(null);
    onSubmit({ category_id: categoryId, per, amount_minor: parsed.minor, valid_from: validFrom });
  }

  return (
    <form className="card form" onSubmit={submit}>
      <h2>{budget ? "Edit Budget" : "New Budget"}</h2>
      <label className="field">
        <span>Expense Category</span>
        <select value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
          {!known && budget && <option value={budget.category_id}>Deleted Category (pick another)</option>}
          {options.map((o) => (
            <option key={o.id} value={o.id}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <label className="field">
          <span>Amount ({currency})</span>
          <input value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" placeholder="400.00" autoFocus />
        </label>
        <label className="field">
          <span>Per</span>
          <select value={per} onChange={(e) => setPer(e.target.value as BudgetPer)}>
            <option value="period">Period (calendar month)</option>
            <option value="week">Week (Monday to Sunday)</option>
          </select>
        </label>
        <label className="field">
          <span>Starts applying on</span>
          <input type="date" value={validFrom} onChange={(e) => setValidFrom(e.target.value)} />
        </label>
      </div>
      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button type="submit" className="primary">
          {budget ? "Save" : "Add Budget"}
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
