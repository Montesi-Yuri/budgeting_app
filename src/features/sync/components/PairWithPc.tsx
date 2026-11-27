import { useState } from 'react'
import { errorMessage, JoinResult, syncApi } from '@/shared/api'
import { scannedPairingCode } from '../lib/pairing'
import { scanPairingCode } from '../lib/scanner'

type Props = {
	/** The phone build: scan the QR code with the camera. */
	mobile: boolean
	onJoined: (joined: JoinResult) => void
	/** "Scan the QR code" or, when rescanning to refresh the PC's address, a different label. */
	scanLabel?: string
}

/**
 * "Pair with PC": scan the PC's pairing QR code with the camera (phone), or
 * paste the pairing code (desktop, or when the camera can't be used). Joins
 * and runs the first Sync. Scanning a PC that is already paired only
 * refreshes its address and keeps the Pairing.
 */
export default function PairWithPc({ mobile, onJoined, scanLabel = 'Pair with PC: scan the QR code' }: Props) {
	const [text, setText] = useState('')
	const [pasting, setPasting] = useState(!mobile)
	const [busy, setBusy] = useState<string | null>(null)
	const [error, setError] = useState<string | null>(null)

	async function join(code: string) {
		setBusy('Pairing and syncing…')
		try {
			const joined = await syncApi.pairingJoin(code)
			setText('')
			onJoined(joined)
		} catch (e) {
			setError(errorMessage(e))
		} finally {
			setBusy(null)
		}
	}

	async function scanAndJoin() {
		setError(null)
		setBusy('Scanning…')
		let scanned
		try {
			scanned = await scanPairingCode()
		} catch (e) {
			// The user closed the scanner, or the camera failed.
			setBusy(null)
			const message = errorMessage(e)
			if (!/cancel/i.test(message)) setError(message)
			return
		}
		if (!scanned.ok) {
			setBusy(null)
			setError(scanned.error)
			return
		}
		await join(scanned.code)
	}

	async function pasteAndJoin() {
		setError(null)
		const checked = scannedPairingCode(text)
		if (!checked.ok) {
			setError(checked.error)
			return
		}
		await join(checked.code)
	}

	return (
		<div className="pair-with-pc">
			{mobile && (
				<button className="primary big" disabled={busy !== null} onClick={() => void scanAndJoin()}>
					{busy ?? scanLabel}
				</button>
			)}
			{mobile && !pasting && (
				<button className="link" onClick={() => setPasting(true)}>
					Paste the pairing code instead
				</button>
			)}
			{pasting && (
				<>
					<p className="muted small">
						On the PC, open Settings → Sync → Pair new device, then
						{' '}
						{mobile ? 'paste' : 'copy and paste'}
						{' '}
						the pairing
						code here.
					</p>
					<textarea rows={2} className="code" value={text} onChange={(e) => setText(e.target.value)} />
					<div className="actions">
						<button disabled={!text.trim() || busy !== null} onClick={() => void pasteAndJoin()}>
							{busy && !mobile ? busy : 'Pair'}
						</button>
					</div>
				</>
			)}
			{error && <p className="error small">{error}</p>}
		</div>
	)
}
