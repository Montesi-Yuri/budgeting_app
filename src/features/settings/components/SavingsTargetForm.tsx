import { useEffect, useState } from 'react'
import { errorMessage, spendableApi } from '@/shared/api'
import { parseAmount, toAmountInput } from '@/shared/lib/money'

/** The Savings target: a fixed amount per Period, set aside from Spendable. */
export default function SavingsTargetForm({ currency }: { currency: string }) {
	const [amount, setAmount] = useState<string | null>(null)
	const [message, setMessage] = useState<{ error: boolean, text: string } | null>(null)
	const [busy, setBusy] = useState(false)

	useEffect(() => {
		spendableApi.savingsTargetGet().then(
			(minor) => setAmount(toAmountInput(minor, currency)),
			(e) => setMessage({ error: true, text: errorMessage(e) }),
		)
	}, [currency])

	async function submit(e: React.FormEvent) {
		e.preventDefault()
		const parsed = parseAmount(amount ?? '', currency)
		if (!parsed.ok) return setMessage({ error: true, text: parsed.error })
		if (parsed.minor < 0) return setMessage({ error: true, text: 'The Savings target can\'t be negative.' })
		setBusy(true)
		try {
			await spendableApi.savingsTargetSet(parsed.minor)
			setAmount(toAmountInput(parsed.minor, currency))
			setMessage({ error: false, text: 'Saved.' })
		} catch (err) {
			setMessage({ error: true, text: errorMessage(err) })
		} finally {
			setBusy(false)
		}
	}

	return (
		<section className="card">
			<h2>Savings target</h2>
			<p className="muted">The amount you intend to save every Period. It is set aside when Spendable is calculated.</p>
			{amount !== null && (
				<form className="inline" onSubmit={submit}>
					<input
						inputMode="decimal"
						aria-label="Savings target per Period"
						value={amount}
						onChange={(e) => setAmount(e.target.value)}
					/>
					<button type="submit" className="primary" disabled={busy}>
						Save
					</button>
				</form>
			)}
			{message && <p className={message.error ? 'error' : 'muted'}>{message.text}</p>}
		</section>
	)
}
