// Pure helpers for the review list and Transfers (no I/O).
import type { TransactionView, TransferView } from '@/shared/api'

/** The "to review" badge: null when there is nothing to review. */
export function badgeText(pendingCount: number): string | null {
	if (pendingCount <= 0) return null
	return pendingCount > 99 ? '99+' : String(pendingCount)
}

/** One side of a Transfer on one line. */
export function sideText(side: TransactionView | null): string {
	if (side === null) return 'Other side not imported yet'
	return `${side.account_name} · ${side.date} · ${side.description}`
}

/**
 * The Transactions `chosen` can be linked with by hand: on another Account,
 * with the opposite sign, and not already in a Transfer with both sides (a
 * Transfer that has only one side is completed by the link).
 */
export function linkCandidates(
	chosen: TransactionView,
	transactions: TransactionView[],
	transfers: TransferView[],
): TransactionView[] {
	const paired = new Set<string>()
	for (const t of transfers) {
		if (t.out_txn_id !== null && t.in_txn_id !== null) {
			paired.add(t.out_txn_id)
			paired.add(t.in_txn_id)
		}
	}
	return transactions.filter(
		(t) =>
			t.account_id !== chosen.account_id
			&& Math.sign(t.amount_minor) === -Math.sign(chosen.amount_minor)
			&& !paired.has(t.id),
	)
}

/** One line of a match's comparison: each version, and what is kept after merging. */
export type MatchRow = { label: string, manual: string, imported: string, merged: string }

/**
 * A Manual Transaction and the imported Transaction it matches, side by side.
 * Merging keeps the imported date, amount and description, and the Manual
 * Transaction's Category and note (the imported ones when it has none).
 */
export function matchRows(manual: TransactionView, imported: TransactionView, money: (minor: number) => string): MatchRow[] {
	const category = (t: TransactionView) => t.category_name ?? 'Uncategorised'
	const note = (t: TransactionView) => t.note ?? ''
	const fromImport = (label: string, value: (t: TransactionView) => string): MatchRow => ({
		label,
		manual: value(manual),
		imported: value(imported),
		merged: value(imported),
	})
	return [
		fromImport('Date', (t) => t.date),
		fromImport('Amount', (t) => money(t.amount_minor)),
		fromImport('Description', (t) => t.description),
		{
			label: 'Category',
			manual: category(manual),
			imported: category(imported),
			merged: category(manual.category_id !== null ? manual : imported),
		},
		{ label: 'Note', manual: note(manual), imported: note(imported), merged: note(manual.note !== null ? manual : imported) },
	]
}
