import { useCallback, useEffect, useState } from 'react'
import { AccountWithBalance, Category, errorMessage, NewRule, Rule, RuleAction, RuleDirection, RuleView, TransactionView, accountsApi, categoriesApi, rulesApi } from '@/shared/api'
import { useAction } from '@/shared/hooks'
import { categoryChoices } from '@/features/categories'
import { TransactionSummary } from '@/features/transactions'
import { ruleSummary } from '../lib/rules'

type Editing = { rule: RuleView | null } | null

/** After a Rule is added or edited: the Transactions it would re-categorise, waiting for the user to confirm. */
type Pending = { rule: Rule, transactions: TransactionView[] } | null

export default function RulesPage({ currency }: { currency: string }) {
  const [rules, setRules] = useState<RuleView[] | null>(null)
  const [categories, setCategories] = useState<Category[]>([])
  const [accounts, setAccounts] = useState<AccountWithBalance[]>([])
  const [editing, setEditing] = useState<Editing>(null)
  const [pending, setPending] = useState<Pending>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const reload = useCallback(() => {
    Promise.all([rulesApi.list(), categoriesApi.list(), accountsApi.list()]).then(([r, c, a]) => {
      setRules(r)
      setCategories(c)
      setAccounts(a)
    }, (e) => setError(errorMessage(e)))
  }, [])
  useEffect(reload, [reload])

  const runAction = useAction(reload, setError)
  const run = (action: () => Promise<unknown>) => {
    setMessage(null)
    return runAction(action)
  }

  async function save(fields: NewRule) {
    const current = editing?.rule ?? null
    const ok = await run(async () => {
      const saved = current ? await rulesApi.update({ ...fields, id: current.id }) : await rulesApi.create(fields)
      const transactions = await rulesApi.recategorisePreview(saved.id)
      setPending(transactions.length > 0 ? { rule: saved, transactions } : null)
    })
    if (ok) setEditing(null)
  }

  async function confirmRecategorise() {
    if (!pending) return
    const { rule, transactions } = pending
    await run(async () => {
      const changed = await rulesApi.recategorise(rule.id, transactions.map((t) => t.id))
      setPending(null)
      setMessage(`${changed} Transaction${changed === 1 ? '' : 's'} re-categorised.`)
    })
  }

  function remove(r: RuleView) {
    if (window.confirm(`Delete the Rule "${ruleSummary(r)}"? Transactions keep their Category.`)) {
      run(() => rulesApi.delete(r.id))
    }
  }

  const noCategories = categories.length === 0

  return (
    <>
      <div className="heading">
        <h1>Rules</h1>
        {editing === null && (
          <button className="primary" onClick={() => setEditing({ rule: null })}>
            Add Rule
          </button>
        )}
      </div>
      <p className="muted small">
        Rules give imported Transactions a Category automatically, or mark them as part of a Transfer. They are tried from the top, and the first matching
        Rule wins. A Category you set by hand is never changed by a Rule.
      </p>
      {error && <p className="error">{error}</p>}
      {message && <p className="muted">{message}</p>}

      {pending && (
        <section className="card">
          <h2>Re-categorise existing Transactions?</h2>
          <p className="muted small">
            The Rule "
            {pending.rule.match_text}
            " matches
            {' '}
            {pending.transactions.length}
            {' '}
            existing Transaction
            {pending.transactions.length === 1 ? '' : 's'}
            . Transactions whose Category you set by hand are not listed
            and won't change.
          </p>
          <ul className="list">
            {pending.transactions.map((t) => (
              <li key={t.id} className="row-item">
                <TransactionSummary t={t} currency={currency} showAccount />
              </li>
            ))}
          </ul>
          <div className="actions">
            <button className="primary" onClick={() => void confirmRecategorise()}>
              Re-categorise
            </button>
            <button onClick={() => setPending(null)}>Leave them as they are</button>
          </div>
        </section>
      )}

      {editing && (
        <RuleForm
          rule={editing.rule}
          categories={categories}
          accounts={accounts}
          onSubmit={(fields) => void save(fields)}
          onCancel={() => setEditing(null)}
        />
      )}

      {rules && rules.length === 0 && !editing && (
        <p className="muted">
          {noCategories
            ? 'No Rules yet. Add a Category to make Rules that assign it, or add a Rule that marks Transfers.'
            : 'No Rules yet. Add one, such as "description contains ESSELUNGA → Groceries".'}
        </p>
      )}

      {rules && rules.length > 0 && (
        <ol className="list">
          {rules.map((r, index) => (
            <li key={r.id} className="row-item">
              <span>
                {ruleSummary(r)}
                {' '}
                {r.broken && <span className="badge broken">Broken: Category deleted</span>}
                {r.broken && <span className="muted small"> Not applied. Edit it to pick another Category.</span>}
              </span>
              <span className="actions">
                <button aria-label="Move up" disabled={index === 0} onClick={() => run(() => rulesApi.move(r.id, index - 1))}>
                  ↑
                </button>
                <button
                  aria-label="Move down"
                  disabled={index === rules.length - 1}
                  onClick={() => run(() => rulesApi.move(r.id, index + 1))}
                >
                  ↓
                </button>
                <button onClick={() => setEditing({ rule: r })}>Edit</button>
                <button className="danger" onClick={() => remove(r)}>
                  Delete
                </button>
              </span>
            </li>
          ))}
        </ol>
      )}
    </>
  )
}

type FormProps = {
  rule: RuleView | null
  categories: Category[]
  accounts: AccountWithBalance[]
  onSubmit: (fields: NewRule) => void
  onCancel: () => void
}

function RuleForm({ rule, categories, accounts, onSubmit, onCancel }: FormProps) {
  const known = rule !== null && !rule.broken && categories.some((c) => c.id === rule.category_id)
  const [matchText, setMatchText] = useState(rule?.match_text ?? '')
  const [accountId, setAccountId] = useState(rule?.account_id ?? '')
  const [direction, setDirection] = useState<RuleDirection | ''>(rule?.direction ?? '')
  const [categoryId, setCategoryId] = useState(known && rule.category_id ? rule.category_id : '')
  const [action, setAction] = useState<RuleAction>(rule?.action ?? 'categorise')

  function submit(e: React.FormEvent) {
    e.preventDefault()
    onSubmit({
      match_text: matchText,
      account_id: accountId || null,
      direction: direction || null,
      action,
      category_id: action === 'categorise' ? categoryId || null : null,
    })
  }

  return (
    <form className="card form" onSubmit={submit}>
      <h2>{rule ? 'Edit Rule' : 'New Rule'}</h2>
      <label className="field">
        <span>Description contains (upper or lower case doesn't matter)</span>
        <input value={matchText} onChange={(e) => setMatchText(e.target.value)} placeholder="ESSELUNGA" autoFocus />
      </label>
      <div className="row">
        <label className="field">
          <span>Account</span>
          <select value={accountId} onChange={(e) => setAccountId(e.target.value)}>
            <option value="">Any Account</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Direction</span>
          <select value={direction} onChange={(e) => setDirection(e.target.value as RuleDirection | '')}>
            <option value="">Money in or out</option>
            <option value="out">Money out only</option>
            <option value="in">Money in only</option>
          </select>
        </label>
        <label className="field">
          <span>Then</span>
          <select value={action} onChange={(e) => setAction(e.target.value as RuleAction)}>
            <option value="categorise">Assign a Category</option>
            <option value="transfer">Mark as Transfer</option>
          </select>
        </label>
        {action === 'categorise' && (
          <label className="field">
            <span>Category</span>
            <select value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
              <option value="" disabled>
                {rule && !known ? 'Deleted Category (pick another)' : 'Choose a Category'}
              </option>
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
        )}
      </div>
      {action === 'transfer' && (
        <p className="muted small">
          Matching Transactions become a confirmed Transfer when they are imported, with only one side until the other
          Account is imported. They are never counted as Income or Expenses.
        </p>
      )}
      <div className="actions">
        <button type="submit" className="primary">
          {rule ? 'Save' : 'Add Rule'}
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  )
}
