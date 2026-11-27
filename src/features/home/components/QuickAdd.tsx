import { useEffect, useState } from 'react'
import { AccountWithBalance, Category, errorMessage, accountsApi, categoriesApi, transactionsApi } from '@/shared/api'
import { formatMinor } from '@/shared/lib/money'
import { defaultAccountId, Direction, quickAmount } from '../lib/quickAdd'
import { localToday } from '@/shared/lib/periods'
import { CategorySelect } from '@/features/categories'

const LAST_ACCOUNT_KEY = 'quickAdd.lastAccount'

// The last used Account is a per-phone convenience: it may be missing.
function readLastAccount(): string | null {
	try {
		return localStorage.getItem(LAST_ACCOUNT_KEY)
	} catch {
		return null
	}
}
function saveLastAccount(id: string) {
	try {
		localStorage.setItem(LAST_ACCOUNT_KEY, id)
	} catch {
		// Not remembered; the main Account is preselected next time.
	}
}

/** The phone's quick "add Manual Transaction" form: amount first, money out by default. */
export default function QuickAdd({ currency, onAdded }: { currency: string, onAdded: () => void }) {
	const [accounts, setAccounts] = useState<AccountWithBalance[] | null>(null)
	const [categories, setCategories] = useState<Category[]>([])
	const [account, setAccount] = useState<string | null>(null)
	const [direction, setDirection] = useState<Direction>('out')
	const [amount, setAmount] = useState('')
	const [description, setDescription] = useState('')
	const [categoryId, setCategoryId] = useState<string | null>(null)
	const [date, setDate] = useState(localToday)
	const [busy, setBusy] = useState(false)
	const [message, setMessage] = useState<string | null>(null)
	const [error, setError] = useState<string | null>(null)

	useEffect(() => {
		Promise.all([accountsApi.list(), categoriesApi.list()]).then(
			([a, c]) => {
				setAccounts(a)
				setCategories(c)
				setAccount(defaultAccountId(a, readLastAccount()))
			},
			(e) => setError(errorMessage(e)),
		)
	}, [])

	async function submit(e: React.FormEvent) {
		e.preventDefault()
		if (!account) return
		const parsed = quickAmount(amount, direction, currency)
		if (!parsed.ok) {
			setError(`Amount: ${parsed.error}`)
			return
		}
		const text = description.trim()
		setBusy(true)
		setError(null)
		try {
			await transactionsApi.create({
				account_id: account,
				date,
				amount_minor: parsed.minor,
				description: text,
				category_id: categoryId,
				note: null,
			})
			saveLastAccount(account)
			setMessage(`Added ${formatMinor(parsed.minor, currency)}${text ? ` · ${text}` : ''}.`)
			setAmount('')
			setDescription('')
			setCategoryId(null)
			setDate(localToday())
			onAdded()
		} catch (err) {
			setError(errorMessage(err))
		} finally {
			setBusy(false)
		}
	}

	if (accounts && accounts.length === 0) {
		return (
			<section className="card">
				<h2>Add a Transaction</h2>
				<p className="muted small">Add an Account first, or pair with your PC to get its Accounts.</p>
			</section>
		)
	}

	const directionButton = (value: Direction, label: string) => (
		<button
			type="button"
			role="radio"
			aria-checked={direction === value}
			className={direction === value ? 'on' : ''}
			onClick={() => setDirection(value)}
		>
			{label}
		</button>
	)

	return (
		<form className="card form quick-add" onSubmit={submit}>
			<h2>Add a Transaction</h2>
			<div className="segmented" role="radiogroup" aria-label="Direction">
				{directionButton('out', 'Money out')}
				{directionButton('in', 'Money in')}
			</div>
			<label className="field">
				<span>
					Amount (
					{currency}
					)
				</span>
				<input
					className="big-input"
					value={amount}
					onChange={(e) => setAmount(e.target.value)}
					inputMode="decimal"
					placeholder="12.50"
				/>
			</label>
			<label className="field">
				<span>Description</span>
				<input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="e.g. Bakery" />
			</label>
			<label className="field">
				<span>Category</span>
				<CategorySelect categories={categories} value={categoryId} onChange={setCategoryId} />
			</label>
			<div className="row">
				<label className="field">
					<span>Account</span>
					<select value={account ?? ''} onChange={(e) => setAccount(e.target.value)}>
						{(accounts ?? []).map((a) => (
							<option key={a.id} value={a.id}>
								{a.name}
							</option>
						))}
					</select>
				</label>
				<label className="field">
					<span>Date</span>
					<input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
				</label>
			</div>
			{error && <p className="error small">{error}</p>}
			{message && <p className="small">{message}</p>}
			<button type="submit" className="primary big" disabled={busy || !account || !amount.trim()}>
				{busy ? 'Adding…' : 'Add'}
			</button>
		</form>
	)
}
