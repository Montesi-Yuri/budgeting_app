import { describe, expect, it } from 'vitest'
import type { Fulfilment, Occurrence, TransactionView } from '@/shared/api'
import { fulfilmentCandidates, linkableOccurrences } from './fulfilment'

const occurrence = (over: Partial<Occurrence> = {}): Occurrence => ({
  id: 'o1',
  planned_item_id: 'salary',
  description: 'Salary',
  account_id: 'current',
  category_id: 'c',
  kind: 'income',
  due_date: '2026-11-27',
  scheduled_date: '2026-11-27',
  amount_minor: 250_000,
  set_aside: false,
  ...over,
})

const txn = (id: string, account_id: string, amount_minor: number): TransactionView => ({
  id,
  account_id,
  date: '2026-11-28',
  amount_minor,
  description: id,
  note: null,
  source: 'import',
  import_id: null,
  fingerprint: null,
  balance_after_minor: null,
  category_id: null,
  category_source: null,
  account_name: account_id,
  category_name: null,
  category_kind: null,
})

const fulfilment = (transaction: TransactionView): Fulfilment => ({
  id: 'o0',
  planned_item_id: 'salary',
  description: 'Salary',
  due_date: '2026-10-27',
  planned_amount_minor: 250_000,
  status: 'confirmed',
  transaction,
})

describe('fulfilmentCandidates: the Transactions an occurrence can be linked to by hand', () => {
  it('offers Transactions on the same Account with the same sign, of any amount', () => {
    const all = [txn('small', 'current', 100), txn('out', 'current', -250_000), txn('other', 'savings', 250_000)]
    expect(fulfilmentCandidates(occurrence(), all, []).map((t) => t.id)).toEqual(['small'])
  })
  it('leaves out Transactions that already fulfil an occurrence', () => {
    const used = txn('used', 'current', 250_000)
    const all = [used, txn('free', 'current', 250_000)]
    expect(fulfilmentCandidates(occurrence(), all, [fulfilment(used)]).map((t) => t.id)).toEqual(['free'])
  })
})

describe('linkableOccurrences: the occurrences that can be linked by hand', () => {
  it('joins this Period\'s and the overdue ones once each, by date', () => {
    const overdue = occurrence({ id: 'oct', due_date: '2026-10-27', scheduled_date: '2026-10-27' })
    const nov = occurrence()
    expect(linkableOccurrences([nov], [overdue, nov]).map((o) => o.id)).toEqual(['oct', 'o1'])
  })
})
