import { useState } from 'react'
import { AccountWithBalance, Category, errorMessage, Transaction, transactionsApi } from '@/shared/api'
import { parseAmount, toAmountInput } from '@/shared/lib/money'
import { CategorySelect } from '@/features/categories'

type Props = {
  currency: string
  accounts: AccountWithBalance[]
  categories: Category[]
  /** The Account preselected for a new Transaction. */
  accountId: string
  /** null = add a new Manual Transaction */
  transaction: Transaction | null
  onDone: () => void
  onCancel: () => void
  /** Shown as a Delete button when editing; a rejection is shown in the form. */
  onDelete?: () => Promise<void>
}

const today = () => new Date().toLocaleDateString('sv-SE') // YYYY-MM-DD in local time

export default function TransactionForm({ currency, accounts, categories, accountId, transaction, onDone, onCancel, onDelete }: Props) {
  const [account, setAccount] = useState(transaction?.account_id ?? accountId)
  const [date, setDate] = useState(transaction?.date ?? today())
  const [amount, setAmount] = useState(transaction ? toAmountInput(transaction.amount_minor, currency) : '')
  const [description, setDescription] = useState(transaction?.description ?? '')
  const [categoryId, setCategoryId] = useState<string | null>(transaction?.category_id ?? null)
  const [note, setNote] = useState(transaction?.note ?? '')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    const parsed = parseAmount(amount, currency)
    if (!parsed.ok) {
      setError(`Amount: ${parsed.error}`)
      return
    }
    // Keep a Category that was deleted elsewhere unless the user picked another one.
    const keptCategory
      = transaction && categoryId === null && !categories.some((c) => c.id === transaction.category_id)
        ? transaction.category_id
        : categoryId
    const fields = {
      account_id: account,
      date,
      amount_minor: parsed.minor,
      description,
      category_id: keptCategory,
      note: note.trim() === '' ? null : note,
    }
    setBusy(true)
    setError(null)
    try {
      if (transaction) await transactionsApi.update({ ...fields, id: transaction.id })
      else await transactionsApi.create(fields)
      onDone()
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  async function remove(onDelete: () => Promise<void>) {
    setBusy(true)
    setError(null)
    try {
      await onDelete()
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="card form" onSubmit={submit}>
      <h2>{transaction ? 'Edit Transaction' : 'New Manual Transaction'}</h2>
      <label className="field">
        <span>Account</span>
        <select value={account} onChange={(e) => setAccount(e.target.value)}>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <label className="field">
          <span>Date</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </label>
        <label className="field">
          <span>
            Amount (
            {currency}
            )
          </span>
          <input
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            inputMode="decimal"
            placeholder="-12.50"
            autoFocus={!transaction}
          />
        </label>
      </div>
      <p className="muted small">Money out is negative (-12.50), money in is positive.</p>
      <label className="field">
        <span>Description</span>
        <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="e.g. Bakery" />
      </label>
      <label className="field">
        <span>Category</span>
        <CategorySelect categories={categories} value={categoryId} onChange={setCategoryId} />
      </label>
      <label className="field">
        <span>Note (optional)</span>
        <input value={note} onChange={(e) => setNote(e.target.value)} />
      </label>
      {error && <p className="error">{error}</p>}
      <div className="actions">
        {onDelete && (
          <button type="button" className="danger delete" onClick={() => remove(onDelete)} disabled={busy}>
            Delete
          </button>
        )}
        <button type="submit" className="primary" disabled={busy}>
          {transaction ? 'Save' : 'Add Transaction'}
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  )
}
