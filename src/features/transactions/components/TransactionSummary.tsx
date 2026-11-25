import type { TransactionView } from '@/shared/api'
import { Money } from '@/shared/ui'
import { FLOW_LABEL, transactionFlow } from '@/features/categories'

/** Date, description, Category (or Uncategorised) and amount of a Transaction. */
export default function TransactionSummary({ t, currency, showAccount }: { t: TransactionView, currency: string, showAccount?: boolean }) {
  const flow = transactionFlow(t.category_kind, t.amount_minor)
  return (
    <>
      <span className="date muted small">{t.date}</span>
      <span className="grow">
        <span className="name">{t.description}</span>
        <span className="muted small">
          {' · '}
          {t.category_name ?? 'Uncategorised'}
          {flow === 'refund' && <span className="badge refund">{FLOW_LABEL.refund}</span>}
          {showAccount && ` · ${t.account_name}`}
          {t.note && ` · ${t.note}`}
        </span>
      </span>
      <Money minor={t.amount_minor} currency={currency} />
    </>
  )
}
