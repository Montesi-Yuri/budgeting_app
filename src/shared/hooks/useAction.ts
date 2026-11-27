import { useCallback } from 'react'
import { errorMessage } from '@/shared/api'

/**
 * Wraps a page's user actions: clears the error, runs the action, then
 * `reload`s on success or shows the error. Resolves to whether it succeeded.
 */
export function useAction(reload: () => void, setError: (error: string | null) => void) {
	return useCallback(
		async (action: () => Promise<unknown>): Promise<boolean> => {
			setError(null)
			try {
				await action()
				reload()
				return true
			} catch (e) {
				setError(errorMessage(e))
				return false
			}
		},
		[reload, setError],
	)
}
