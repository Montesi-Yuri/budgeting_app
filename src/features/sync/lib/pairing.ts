// Pure helpers for pairing with and syncing to the PC from the phone (ticket 15):
// the QR scanner's result and the "Sync now" target.
import type { CommandError, Peer } from '@/shared/api'

const PAIRING_PREFIX = 'bsync:'

export type Scanned = { ok: true, code: string } | { ok: false, error: string }

/** Check what the camera read before trying to pair with it. */
export function scannedPairingCode(content: string): Scanned {
	const code = content.trim()
	if (code.startsWith(PAIRING_PREFIX)) return { ok: true, code }
	return {
		ok: false,
		error: 'This QR code is not a pairing code. On the PC, open Settings → Sync → Pair new device and scan the code shown there.',
	}
}

/** A Sync failed because the PC couldn't be reached: offer to rescan its QR code. */
export function isUnreachable(err: unknown): boolean {
	return typeof err === 'object' && err !== null && (err as CommandError).kind === 'unreachable'
}

/** The Device "Sync now" connects to: one with a known address, the most recently synced first. */
export function syncTarget(peers: Peer[]): Peer | null {
	const reachable = peers.filter((p) => p.last_addr !== null)
	reachable.sort((a, b) => (b.last_sync_at ?? 0) - (a.last_sync_at ?? 0))
	return reachable[0] ?? null
}
