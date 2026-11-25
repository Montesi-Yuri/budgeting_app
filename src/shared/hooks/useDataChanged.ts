import { useEffect, useEffectEvent } from 'react'
import { listen } from '@tauri-apps/api/event'

export type DataChanged = { entities: string[] }

/**
 * Calls `handler` whenever the shell emits "data://changed" (a Sync batch was
 * applied). The latest `handler` is used, so it needn't be memoised.
 */
export function useDataChanged(handler: (payload: DataChanged) => void) {
  const onChange = useEffectEvent(handler)

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    listen<DataChanged>('data://changed', ({ payload }) => onChange(payload))
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch(() => undefined)
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])
}
