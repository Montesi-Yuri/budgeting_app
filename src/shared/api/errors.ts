/** What a failed command rejects with. */
export type CommandError = {
  kind: 'invalid' | 'not_found' | 'internal' | 'sync' | 'unreachable' | 'firewall'
  message: string
}

export function errorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'message' in err) return String((err as CommandError).message)
  return String(err)
}
