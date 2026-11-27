// Pure helpers for showing Categories and Transactions (no I/O).
import type { Category, CategoryKind } from '@/shared/api'

/**
 * Whether a Transaction is Income, an Expense or a Refund. This is decided by
 * its Category's kind, never by the sign of the amount: a positive amount in
 * an expense Category is a Refund, not Income.
 */
export function transactionFlow(
	kind: CategoryKind | null,
	amountMinor: number,
): 'income' | 'expense' | 'refund' | 'uncategorised' {
	if (kind === null) return 'uncategorised'
	if (kind === 'income') return 'income'
	return amountMinor > 0 ? 'refund' : 'expense'
}

export const FLOW_LABEL = {
	income: 'Income',
	expense: 'Expense',
	refund: 'Refund',
	uncategorised: 'Uncategorised',
} as const

export const KIND_LABEL: Record<CategoryKind, string> = { expense: 'Expense', income: 'Income' }

export type CategoryChoiceGroup = { kind: CategoryKind, label: string, options: { id: string, label: string }[] }

/**
 * The Categories for a picker, grouped by kind (expense first), in the order
 * categories_list returns them. Children are labelled "Parent › Child".
 */
export function categoryChoices(categories: Category[]): CategoryChoiceGroup[] {
	const byId = new Map(categories.map((c) => [c.id, c]))
	return (['expense', 'income'] as const)
		.map((kind) => ({
			kind,
			label: KIND_LABEL[kind],
			options: categories
				.filter((c) => c.kind === kind)
				.map((c) => {
					const parent = c.parent_id ? byId.get(c.parent_id) : undefined
					return { id: c.id, label: parent ? `${parent.name} › ${c.name}` : c.name }
				}),
		}))
		.filter((group) => group.options.length > 0)
}

/** Where a Category sits among its siblings (same parent), for move up/down. */
export function siblingPlace(categories: Category[], id: string): { index: number, count: number } {
	const self = categories.find((c) => c.id === id)
	const siblings = categories.filter((c) => c.parent_id === (self?.parent_id ?? null))
	return { index: siblings.findIndex((c) => c.id === id), count: siblings.length }
}
