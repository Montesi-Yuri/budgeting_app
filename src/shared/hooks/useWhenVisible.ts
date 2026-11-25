import { useEffect, useEffectEvent } from 'react'

/**
 * Calls `handler` when the user returns to the app: the window gets the focus
 * or the page becomes visible again. The latest `handler` is used.
 */
export function useWhenVisible(handler: () => void) {
  const onReturn = useEffectEvent(handler)

  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === 'visible') onReturn()
    }
    window.addEventListener('focus', onVisible)
    document.addEventListener('visibilitychange', onVisible)
    return () => {
      window.removeEventListener('focus', onVisible)
      document.removeEventListener('visibilitychange', onVisible)
    }
  }, [])
}
