import { useCallback, useEffect, useRef, useState } from 'react'
import { errorMessage, JoinResult, PeriodSpendable, Peer, spendableApi, syncApi } from '@/shared/api'
import { formatMinor } from '@/shared/lib/money'
import { dayLabel, localToday, periodLabel, periodOf } from '@/shared/lib/periods'
import { useDataChanged } from '@/shared/hooks'
import { isUnreachable, lastSyncText, PairWithPc, shouldAutoSync, syncSummary, syncTarget } from '@/features/sync'
import QuickAdd from './QuickAdd'

/**
 * The phone's main screen: Spendable, Weekly Spendable, quick add and Sync with the PC. When the app comes to
 * the foreground it tries a quick automatic Sync with the PC, and stays quiet if the PC can't be reached.
 */
export default function PhoneHome({ currency }: { currency: string }) {
	const [data, setData] = useState<PeriodSpendable | null>(null)
	const [peers, setPeers] = useState<Peer[] | null>(null)
	const [syncing, setSyncing] = useState(false)
	const [message, setMessage] = useState<string | null>(null)
	const [error, setError] = useState<string | null>(null)
	const [rescan, setRescan] = useState(false)
	// Bumped after a Sync received changes, so quick add reloads the Accounts and Categories.
	const [synced, setSynced] = useState(0)
	const [now, setNow] = useState(() => Date.now())
	// Read by the visibilitychange handler, which is set up once.
	const peersRef = useRef<Peer[] | null>(null)
	const syncingRef = useRef(false)
	const lastAttemptRef = useRef<number | null>(null)

	const reload = useCallback(() => {
		const today = localToday()
		spendableApi.spendableGet(periodOf(today), today).then(setData, (e) => setError(errorMessage(e)))
	}, [])
	const loadPeers = useCallback(
		() =>
			syncApi.peers().then(
				(list) => {
					peersRef.current = list
					setPeers(list)
					return list
				},
				(e) => {
					setError(errorMessage(e))
					return null
				},
			),
		[],
	)

	/** The automatic Sync: quick, and silent when the PC can't be reached. */
	const autoSync = useCallback(async () => {
		const target = peersRef.current ? syncTarget(peersRef.current) : null
		const nowMs = Date.now()
		if (!shouldAutoSync(target, { syncing: syncingRef.current, lastAttemptMs: lastAttemptRef.current }, nowMs)) return
		lastAttemptRef.current = nowMs
		syncingRef.current = true
		setSyncing(true)
		try {
			await syncApi.auto(target!.device_id)
			setError(null)
			setRescan(false)
		} catch {
			// Not on the same network, or Sync is off on the PC: "Sync now" still says why.
		} finally {
			syncingRef.current = false
			setSyncing(false)
			setNow(Date.now())
			void loadPeers()
		}
	}, [loadPeers])

	useEffect(() => {
		reload()
		void loadPeers().then(() => autoSync())
		const onVisible = () => {
			if (document.visibilityState !== 'visible') return
			reload()
			setNow(Date.now())
			void autoSync()
		}
		const clock = setInterval(() => setNow(Date.now()), 30_000)
		document.addEventListener('visibilitychange', onVisible)
		return () => {
			document.removeEventListener('visibilitychange', onVisible)
			clearInterval(clock)
		}
	}, [reload, loadPeers, autoSync])

	useDataChanged(() => {
		reload()
		setSynced((n) => n + 1)
	})

	const target = peers ? syncTarget(peers) : null

	async function syncNow() {
		if (!target || syncingRef.current) return
		syncingRef.current = true
		setSyncing(true)
		setError(null)
		setMessage(null)
		setRescan(false)
		try {
			setMessage(syncSummary(await syncApi.now(target.device_id)))
		} catch (e) {
			setError(errorMessage(e))
			setRescan(isUnreachable(e))
		} finally {
			syncingRef.current = false
			setSyncing(false)
			setNow(Date.now())
			void loadPeers()
		}
	}

	function joined(j: JoinResult) {
		setError(null)
		setRescan(false)
		setMessage(`Paired with ${j.peer.name}. ${syncSummary(j.report)}`)
		void loadPeers()
	}

	const money = (minor: number) => (
		<span className={minor < 0 ? 'amount negative' : 'amount'}>{formatMinor(minor, currency)}</span>
	)

	return (
		<div className="phone-home">
			{data && (
				<section className="card phone-spendable">
					<div className="muted small">
						Spendable ·
						{periodLabel(data.period)}
					</div>
					<div className="headline">{money(data.spendable_minor)}</div>
					{data.weekly && (
						<div className="phone-weekly">
							<div className="muted small">
								Weekly Spendable ·
								{' '}
								{dayLabel(data.weekly.week_first_day)}
								{' '}
								–
								{' '}
								{dayLabel(data.weekly.week_last_day)}
							</div>
							<div className="headline small-headline">{money(data.weekly.amount_minor)}</div>
						</div>
					)}
				</section>
			)}

			<QuickAdd key={synced} currency={currency} onAdded={reload} />

			<section className="card">
				<h2>Sync</h2>
				{peers === null
					? null
					: target
						? (
								<>
									<button className="primary big" disabled={syncing} onClick={() => void syncNow()}>
										{syncing ? 'Syncing…' : `Sync now with ${target.name}`}
									</button>
									<p className="muted small">
										Last Sync:
										{' '}
										{lastSyncText(target.last_sync_at, now)}
										. It syncs by itself when you open the app and the
										PC is reachable.
									</p>
								</>
							)
						: (
								<>
									<p className="muted small">Pair with your PC to Sync. On the PC, open Settings → Sync → Pair new device.</p>
									<PairWithPc mobile onJoined={joined} />
								</>
							)}
				{message && <p className="small">{message}</p>}
				{error && <p className="error small">{error}</p>}
				{rescan && <PairWithPc mobile onJoined={joined} scanLabel="Rescan the PC's QR code (keeps the Pairing)" />}
			</section>
		</div>
	)
}
