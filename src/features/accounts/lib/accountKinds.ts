import type { AccountKind } from '@/shared/api'

export const ACCOUNT_KINDS: { kind: AccountKind, label: string }[] = [
  { kind: 'current', label: 'Current account' },
  { kind: 'savings', label: 'Savings account' },
  { kind: 'credit_card', label: 'Credit card' },
]
