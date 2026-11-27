import { invoke } from '@tauri-apps/api/core'
import type { Occurrence } from './planned'
import type { TransactionView } from './transactions'

/** Where a Suggestion stands. "suggested" counts as confirmed until the user decides. */
export type LinkStatus = 'suggested' | 'confirmed' | 'rejected'

/** A Transfer between two of the user's Accounts. A side is null until that Account is imported. */
export type Transfer = {
	id: string
	out_txn_id: string | null
	in_txn_id: string | null
	status: LinkStatus
	origin: 'rule' | 'match' | 'user'
}

/** A Transfer with its live Transactions (null = not imported yet, or deleted). */
export type TransferView = Transfer & { out_txn: TransactionView | null, in_txn: TransactionView | null }

/** A link between a Manual Transaction and the imported Transaction it duplicates. */
export type ManualMatch = { id: string, manual_txn_id: string, imported_txn_id: string, status: LinkStatus }

/** A match with both versions, side by side. */
export type ManualMatchView = ManualMatch & { manual_txn: TransactionView, imported_txn: TransactionView }

/** A fulfilled occurrence: the Planned item next to its actual Transaction. */
export type Fulfilment = {
	/** The occurrence id. */
	id: string
	planned_item_id: string
	description: string
	/** The schedule's date of the occurrence. */
	due_date: string
	planned_amount_minor: number
	status: LinkStatus
	transaction: TransactionView
}

export type ReviewItem =
	| { kind: 'transfer_suggestion', transfer: TransferView }
  /** A Transfer with one side whose Period has ended: "other side not imported yet". */
	| { kind: 'transfer_missing_side', transfer: TransferView }
  /** An imported Transaction suggested as the actual of a Planned item's occurrence. */
	| { kind: 'fulfilment_suggestion', fulfilment: Fulfilment }
  /** An occurrence still expected after its Period ended: "skipped or arrives later?". */
	| { kind: 'still_expected', occurrence: Occurrence }
  /** An imported Transaction that looks like a Manual Transaction: confirm (merge) or reject. */
	| { kind: 'manual_match_suggestion', manual_match: ManualMatchView }

/** pending_count: the items waiting for a decision (the badge). */
export type ReviewList = { items: ReviewItem[], pending_count: number }

export const reviewApi = {
	/** The review list as of `today` (YYYY-MM-DD). */
	list: (today: string) => invoke<ReviewList>('review_list', { today }),
}

export const transfersApi = {
	/** Every Transfer that isn't rejected, oldest first. */
	list: () => invoke<TransferView[]>('transfers_list'),
	/** Link two Transactions (one out, one in, different Accounts) as a Transfer by hand. */
	link: (a: string, b: string) => invoke<Transfer>('transfer_link', { a, b }),
	confirm: (id: string) => invoke<Transfer>('transfer_confirm', { id }),
	/** Reject a Suggestion, or unlink a Transfer. The pair is never suggested again. */
	reject: (id: string) => invoke<Transfer>('transfer_reject', { id }),
}

export const manualMatchesApi = {
	/** Merge a matched Manual Transaction into the imported one (its Category and note move over). */
	confirm: (id: string) => invoke<ManualMatch>('manual_match_confirm', { id }),
	/** Keep both Transactions. The pair is never suggested again. */
	reject: (id: string) => invoke<ManualMatch>('manual_match_reject', { id }),
}

export const fulfilmentApi = {
	/** Every fulfilled occurrence (suggested or confirmed), oldest first. */
	list: () => invoke<Fulfilment[]>('fulfilments_list'),
	/** Link a Transaction by hand to the occurrence due on dueDate (the schedule's date). */
	link: (plannedItemId: string, dueDate: string, txnId: string) =>
		invoke<unknown>('fulfilment_link', { plannedItemId, dueDate, txnId }),
	confirm: (id: string) => invoke<unknown>('fulfilment_confirm', { id }),
	/** Reject a Suggestion or unlink: the occurrence is expected again; never suggested again. */
	unlink: (id: string) => invoke<unknown>('fulfilment_unlink', { id }),
	skip: (plannedItemId: string, dueDate: string) => invoke<unknown>('occurrence_skip', { plannedItemId, dueDate }),
	/** It is now expected on newDate (after dueDate). */
	arrivesLater: (plannedItemId: string, dueDate: string, newDate: string) =>
		invoke<unknown>('occurrence_arrives_later', { plannedItemId, dueDate, newDate }),
}
