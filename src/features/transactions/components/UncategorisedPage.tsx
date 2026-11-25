import { useCallback, useEffect, useState } from 'react'
import { Category, errorMessage, TransactionView, categoriesApi, transactionsApi } from '@/shared/api'
import { CategorySelect } from '@/features/categories'
import TransactionSummary from './TransactionSummary'

/** Every Transaction without a Category, on any Account; pick a Category to assign it. */
export default function UncategorisedPage({ currency }: { currency: string }) {
  const [transactions, setTransactions] = useState<TransactionView[] | null>(null)
  const [categories, setCategories] = useState<Category[]>([])
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const reload = useCallback(() => {
    transactionsApi.uncategorised().then(setTransactions, (e) => setError(errorMessage(e)))
  }, [])
  useEffect(() => {
    reload()
    categoriesApi.list().then(setCategories, (e) => setError(errorMessage(e)))
  }, [reload])

  async function assign(id: string, categoryId: string | null) {
    if (!categoryId) return
    setBusy(id)
    setError(null)
    try {
      await transactionsApi.setCategory(id, categoryId)
      reload()
    } catch (e) {
      setError(errorMessage(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <>
      <h1>Uncategorised</h1>
      <p className="muted small">
        These Transactions have no Category yet. They still count towards your spending. A Category you choose here
        is never changed by a Rule.
      </p>
      {error && <p className="error">{error}</p>}
      {transactions && transactions.length === 0 && <p className="muted">Every Transaction has a Category.</p>}
      {transactions && transactions.length > 0 && (
        <ul className="list">
          {transactions.map((t) => (
            <li key={t.id} className="row-item">
              <TransactionSummary t={t} currency={currency} showAccount />
              <CategorySelect
                aria-label={`Category for ${t.description}`}
                categories={categories}
                value={null}
                disabled={busy === t.id}
                onChange={(categoryId) => assign(t.id, categoryId)}
              />
            </li>
          ))}
        </ul>
      )}
    </>
  )
}
