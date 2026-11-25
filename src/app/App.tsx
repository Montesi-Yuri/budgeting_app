import { useEffect, useState } from 'react'
import { errorMessage, reviewApi, settingsApi } from '@/shared/api'
import { localToday } from '@/shared/lib/periods'
import { useDataChanged } from '@/shared/hooks'
import { badgeText, ReviewPage } from '@/features/review'
import { AccountsPage } from '@/features/accounts'
import { BudgetsPage } from '@/features/budgets'
import { CategoriesPage } from '@/features/categories'
import { SettingsPage } from '@/features/settings'
import { ForecastPage } from '@/features/forecast'
import { ImportPage } from '@/features/imports'
import { HomePage, PhoneHome } from '@/features/home'
import { PlannedItemsPage } from '@/features/planned'
import { RulesPage } from '@/features/rules'
import { TransactionsPage, UncategorisedPage } from '@/features/transactions'
import { PHONE_VIEWS, View, VIEWS, viewsToRefresh } from './views'
import Welcome from './Welcome'

export default function App() {
  // undefined = loading, null = not chosen yet (first launch)
  const [currency, setCurrency] = useState<string | null | undefined>(undefined)
  const [view, setView] = useState<View>('home')
  const [error, setError] = useState<string | null>(null)
  // The phone build gets the phone layout and the QR scanner.
  const [mobile, setMobile] = useState(false)

  // Bumped when a Sync changed what a view shows, so the view refetches.
  const [refreshed, setRefreshed] = useState<Partial<Record<View, number>>>({})

  // The "to review" badge: refreshed on every navigation, after a Sync and
  // after a decision on the review page.
  const [pending, setPending] = useState(0)
  const loadPending = () => reviewApi.list(localToday()).then((r) => setPending(r.pending_count), () => {})
  useEffect(() => {
    if (currency) void loadPending()
  }, [currency, view])

  const loadCurrency = () => settingsApi.currencyGet().then(setCurrency, (e) => setError(errorMessage(e)))
  useEffect(() => {
    void loadCurrency()
    settingsApi.appInfo().then((info) => setMobile(info.mobile), () => {})
  }, [])

  useDataChanged(({ entities }) => {
    if (entities.includes('setting')) settingsApi.currencyGet().then(setCurrency, () => {})
    void loadPending()
    setRefreshed((r) => {
      const next = { ...r }
      for (const v of viewsToRefresh(entities)) next[v] = (next[v] ?? 0) + 1
      return next
    })
  })

  async function chooseCurrency(code: string) {
    await settingsApi.currencySet(code)
    setCurrency(code)
  }

  if (error) return <main className="page"><p className="error">{error}</p></main>
  if (currency === undefined) return <main className="page"><p className="muted">Loading…</p></main>

  if (currency === null) return <Welcome mobile={mobile} onReady={() => void loadCurrency()} onCurrencyChosen={chooseCurrency} />

  return (
    <div className={mobile ? 'app phone' : 'app'}>
      <header className="topbar">
        <span className="brand">Budget</span>
        <nav>
          {VIEWS.filter((v) => !mobile || PHONE_VIEWS.includes(v.view)).map((v) => (
            <button key={v.view} className={view === v.view ? 'tab active' : 'tab'} onClick={() => setView(v.view)}>
              {v.label}
              {v.view === 'review' && badgeText(pending) && (
                <span className="badge" aria-label={`${pending} to review`}>
                  {badgeText(pending)}
                </span>
              )}
            </button>
          ))}
        </nav>
      </header>
      <main className="page" key={`${view}-${refreshed[view] ?? 0}`}>
        {view === 'home' && (mobile ? <PhoneHome currency={currency} /> : <HomePage currency={currency} />)}
        {view === 'forecast' && <ForecastPage currency={currency} />}
        {view === 'review' && <ReviewPage currency={currency} onChanged={() => void loadPending()} />}
        {view === 'accounts' && <AccountsPage currency={currency} />}
        {view === 'transactions' && <TransactionsPage currency={currency} />}
        {view === 'uncategorised' && <UncategorisedPage currency={currency} />}
        {view === 'categories' && <CategoriesPage />}
        {view === 'settings' && <SettingsPage currency={currency} onCurrencyChange={chooseCurrency} onRestored={() => void loadCurrency()} />}
        {view === 'budgets' && <BudgetsPage currency={currency} />}
        {view === 'import' && <ImportPage currency={currency} />}
        {view === 'planned' && <PlannedItemsPage currency={currency} />}
        {view === 'rules' && <RulesPage currency={currency} />}
      </main>
    </div>
  )
}
