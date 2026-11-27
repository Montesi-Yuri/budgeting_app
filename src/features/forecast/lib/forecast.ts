// Pure helpers for showing the Forecast (no I/O).

/** Complete Periods of history below which the Forecast is low confidence. */
export const HISTORY_PERIODS = 3

/**
 * Bar lengths (0-100) for the month-by-month Net position: the largest
 * amount, by size, is a full bar. A negative Net position is marked so it
 * can be drawn in the negative colour. A non-zero amount is never shorter
 * than 1%, so it stays visible.
 */
export function netBars(amounts: number[]): { percent: number, negative: boolean }[] {
	const largest = Math.max(0, ...amounts.map(Math.abs))
	return amounts.map((amount) => {
		const percent = largest === 0 ? 0 : Math.round((Math.abs(amount) / largest) * 100)
		return { percent: amount !== 0 ? Math.max(1, percent) : 0, negative: amount < 0 }
	})
}

/** Why the Forecast is low confidence, or null when it isn't. */
export function confidenceText(completePeriods: number): string | null {
	if (completePeriods >= HISTORY_PERIODS) return null
	if (completePeriods === 0) return 'Low confidence: no complete Period of history yet.'
	const periods = completePeriods === 1 ? '1 complete Period' : `${completePeriods} complete Periods`
	return `Low confidence: only ${periods} of history.`
}
