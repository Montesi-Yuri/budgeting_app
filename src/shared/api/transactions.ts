import { invoke } from '@tauri-apps/api/core'
import type { CategoryKind } from './categories'

export type NewManualTransaction = {
	account_id: string
	/** YYYY-MM-DD */
	date: string
	/** Signed minor units: negative = money out of the Account. */
	amount_minor: number
	description: string
	category_id: string | null
	note: string | null
}

export type TransactionEdit = NewManualTransaction & { id: string }

export type Transaction = TransactionEdit & {
	source: 'import' | 'manual'
	import_id: string | null
	fingerprint: string | null
	balance_after_minor: number | null
	/** Who set the Category; Rules never overwrite "user". */
	category_source: 'user' | 'rule' | null
}

/** A Transaction in a list. A null category_name means Uncategorised. */
export type TransactionView = Transaction & {
	account_name: string
	category_name: string | null
	category_kind: CategoryKind | null
}

export const transactionsApi = {
	list: (accountId: string) => invoke<TransactionView[]>('transactions_list', { account: accountId }),
	uncategorised: () => invoke<TransactionView[]>('transactions_uncategorised'),
	create: (transaction: NewManualTransaction) => invoke<Transaction>('transaction_create', { transaction }),
	update: (transaction: TransactionEdit) => invoke<Transaction>('transaction_update', { transaction }),
	/** Set (or clear, with null) a Category by hand. */
	setCategory: (id: string, categoryId: string | null) =>
		invoke<Transaction>('transaction_set_category', { id, category: categoryId }),
	delete: (id: string) => invoke<void>('transaction_delete', { id }),
}
