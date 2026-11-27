import { useEffect, useMemo, useState } from 'react'
import { errorMessage, ManualMatchView, ReviewList, TransactionView, TransferView, accountsApi, manualMatchesApi, reviewApi, transactionsApi, transfersApi } from '@/shared/api'
import { formatMinor } from '@/shared/lib/money'
import { localToday } from '@/shared/lib/periods'
import { Money } from '@/shared/ui'
import { linkCandidates, matchRows, sideText } from '../lib/review'
import PlannedReview from './PlannedReview'

const ORIGIN_LABEL: Record<TransferView['origin'], string> = {
	rule: 'by a Rule',
	match: 'suggested',
	user: 'linked by hand',
}

type ReviewData = { review: ReviewList, transfers: TransferView[], transactions: TransactionView[] }

async function fetchReviewData(): Promise<ReviewData> {
	const [review, transfers, accounts] = await Promise.all([reviewApi.list(localToday()), transfersApi.list(), accountsApi.list()])
	const lists = await Promise.all(accounts.map((a) => transactionsApi.list(a.id)))
	return { review, transfers, transactions: lists.flat().sort((a, b) => b.date.localeCompare(a.date)) }
}

/** The review list (Suggestions to confirm or reject) and the Transfers. */
export default function ReviewPage({ currency, onChanged }: { currency: string, onChanged: () => void }) {
	const [review, setReview] = useState<ReviewList | null>(null)
	const [transfers, setTransfers] = useState<TransferView[]>([])
	const [transactions, setTransactions] = useState<TransactionView[]>([])
	const [error, setError] = useState<string | null>(null)

	function show(data: ReviewData) {
		setReview(data.review)
		setTransfers(data.transfers)
		setTransactions(data.transactions)
		setError(null)
	}
	async function load() {
		try {
			show(await fetchReviewData())
		} catch (e) {
			setError(errorMessage(e))
		}
	}
	useEffect(() => {
		fetchReviewData().then(show, (e) => setError(errorMessage(e)))
	}, [])

	async function run(action: () => Promise<unknown>) {
		try {
			await action()
			onChanged()
			await load()
		} catch (e) {
			setError(errorMessage(e))
		}
	}

	const suggestions = review?.items.filter((i) => i.kind === 'transfer_suggestion') ?? []
	const missing = review?.items.filter((i) => i.kind === 'transfer_missing_side') ?? []
	const matches = review?.items.flatMap((i) => (i.kind === 'manual_match_suggestion' ? [i.manual_match] : [])) ?? []

	return (
		<>
			<h1>To review</h1>
			<p className="muted">
				Suggestions are links the app proposes. Until you decide, they count as confirmed. A rejected pair is never
				suggested again.
			</p>
			{error && <p className="error">{error}</p>}

			<section className="card">
				<h2>Suggested Transfers</h2>
				{review && suggestions.length === 0 && <p className="muted small">Nothing to review.</p>}
				<ul className="list">
					{suggestions.map(({ transfer }) => (
						<li key={transfer.id} className="row-item">
							<TransferSides transfer={transfer} currency={currency} />
							<span className="actions">
								<button className="primary" onClick={() => run(() => transfersApi.confirm(transfer.id))}>
									Confirm
								</button>
								<button onClick={() => run(() => transfersApi.reject(transfer.id))}>Reject</button>
							</span>
						</li>
					))}
				</ul>
			</section>

			{matches.length > 0 && (
				<section className="card">
					<h2>Entered by hand, then imported</h2>
					<p className="muted small">
						These imported Transactions look like ones you entered by hand. Until you decide, only the imported one
						counts. Confirm merges them: the imported date and amount win, and your Category and note are kept.
					</p>
					<ul className="list">
						{matches.map((m) => (
							<li key={m.id} className="row-item">
								<MatchComparison match={m} currency={currency} />
								<span className="actions">
									<button className="primary" onClick={() => run(() => manualMatchesApi.confirm(m.id))}>
										Merge
									</button>
									<button onClick={() => run(() => manualMatchesApi.reject(m.id))}>Keep both</button>
								</span>
							</li>
						))}
					</ul>
				</section>
			)}

			{missing.length > 0 && (
				<section className="card">
					<h2>Other side not imported yet</h2>
					<p className="muted small">
						These Transfers have only one side, and their Period has ended. They still count as Transfers. Import the
						other Account to complete them.
					</p>
					<ul className="list">
						{missing.map(({ transfer }) => (
							<li key={transfer.id} className="row-item">
								<TransferSides transfer={transfer} currency={currency} />
								<span className="actions">
									<button onClick={() => run(() => transfersApi.reject(transfer.id))}>Unlink</button>
								</span>
							</li>
						))}
					</ul>
				</section>
			)}

			<PlannedReview currency={currency} review={review} transactions={transactions} run={run} />

			<LinkForm currency={currency} transactions={transactions} transfers={transfers} onLink={(a, b) => run(() => transfersApi.link(a, b))} />

			<section className="card">
				<h2>Transfers</h2>
				{transfers.length === 0 && <p className="muted small">No Transfers yet.</p>}
				<ul className="list">
					{transfers.map((t) => (
						<li key={t.id} className="row-item">
							<TransferSides transfer={t} currency={currency} />
							<span className="muted small">
								{t.status === 'suggested' ? 'to review' : ORIGIN_LABEL[t.origin]}
							</span>
							<span className="actions">
								<button onClick={() => run(() => transfersApi.reject(t.id))}>Unlink</button>
							</span>
						</li>
					))}
				</ul>
			</section>
		</>
	)
}

function MatchComparison({ match, currency }: { match: ManualMatchView, currency: string }) {
	const rows = matchRows(match.manual_txn, match.imported_txn, (minor) => formatMinor(minor, currency))
	return (
		<table className="grow compare">
			<caption className="name">{match.imported_txn.account_name}</caption>
			<thead>
				<tr>
					<th />
					<th>Entered by hand</th>
					<th>Imported</th>
					<th>After merging</th>
				</tr>
			</thead>
			<tbody>
				{rows.map((r) => (
					<tr key={r.label}>
						<th scope="row">{r.label}</th>
						<td>{r.manual}</td>
						<td>{r.imported}</td>
						<td>{r.merged}</td>
					</tr>
				))}
			</tbody>
		</table>
	)
}

function TransferSides({ transfer, currency }: { transfer: TransferView, currency: string }) {
	const amount = transfer.out_txn ? -transfer.out_txn.amount_minor : (transfer.in_txn?.amount_minor ?? 0)
	return (
		<span className="grow">
			<span className="name">
				From:
				{sideText(transfer.out_txn)}
			</span>
			<br />
			<span className="name">
				To:
				{sideText(transfer.in_txn)}
			</span>
			{' '}
			<Money minor={amount} currency={currency} />
		</span>
	)
}

type LinkProps = {
	currency: string
	transactions: TransactionView[]
	transfers: TransferView[]
	onLink: (a: string, b: string) => void
}

function LinkForm({ currency, transactions, transfers, onLink }: LinkProps) {
	const [firstId, setFirstId] = useState('')
	const [secondId, setSecondId] = useState('')
	const first = transactions.find((t) => t.id === firstId) ?? null
	// Any Transaction not yet in a Transfer with both sides can start a link.
	const linkable = useMemo(() => {
		const paired = new Set(
			transfers.filter((t) => t.out_txn_id && t.in_txn_id).flatMap((t) => [t.out_txn_id, t.in_txn_id]),
		)
		return transactions.filter((t) => !paired.has(t.id))
	}, [transactions, transfers])
	const candidates = first ? linkCandidates(first, transactions, transfers) : []
	const label = (t: TransactionView) => `${sideText(t)} · ${formatMinor(t.amount_minor, currency)}`

	function submit(e: React.FormEvent) {
		e.preventDefault()
		if (!firstId || !secondId) return
		onLink(firstId, secondId)
		setFirstId('')
		setSecondId('')
	}

	return (
		<form className="card form" onSubmit={submit}>
			<h2>Link a Transfer by hand</h2>
			<label className="field">
				<span>Transaction</span>
				<select
					value={firstId}
					onChange={(e) => {
						setFirstId(e.target.value)
						setSecondId('')
					}}
				>
					<option value="">Choose a Transaction</option>
					{linkable.map((t) => (
						<option key={t.id} value={t.id}>
							{label(t)}
						</option>
					))}
				</select>
			</label>
			<label className="field">
				<span>Its other side (another Account, the opposite direction)</span>
				<select value={secondId} onChange={(e) => setSecondId(e.target.value)} disabled={!first}>
					<option value="">Choose the other side</option>
					{candidates.map((t) => (
						<option key={t.id} value={t.id}>
							{label(t)}
						</option>
					))}
				</select>
			</label>
			<div className="actions">
				<button type="submit" className="primary" disabled={!firstId || !secondId}>
					Link as Transfer
				</button>
			</div>
		</form>
	)
}
