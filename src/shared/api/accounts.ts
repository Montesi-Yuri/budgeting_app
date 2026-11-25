import { invoke } from '@tauri-apps/api/core'

export type AccountKind = 'current' | 'savings' | 'credit_card'

export type NewAccount = {
  name: string
  kind: AccountKind
  /** Opening balance in minor units; credit card debt is negative. */
  opening_minor: number
  /** YYYY-MM-DD */
  opening_date: string
  is_main: boolean
}

export type Account = NewAccount & { id: string }

export type AccountWithBalance = Account & { balance_minor: number }

export const accountsApi = {
  list: () => invoke<AccountWithBalance[]>('accounts_list'),
  create: (account: NewAccount) => invoke<Account>('account_create', { account }),
  update: (account: Account) => invoke<Account>('account_update', { account }),
  delete: (id: string) => invoke<void>('account_delete', { id }),
}
