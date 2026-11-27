// Periods (calendar months, "YYYY-MM") and dates ("YYYY-MM-DD") on the
// user's local clock. The Spendable calculation itself is in budget_core.

const pad = (n: number, width = 2) => String(n).padStart(width, '0')

/** The local calendar date of `now` as YYYY-MM-DD. */
export function localToday(now: Date = new Date()): string {
	return `${pad(now.getFullYear(), 4)}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
}

/** The Period (YYYY-MM) of a YYYY-MM-DD date. */
export function periodOf(date: string): string {
	return date.slice(0, 7)
}

/** The Period `delta` months before (negative) or after `period`. */
export function shiftPeriod(period: string, delta: number): string {
	const [year, month] = period.split('-').map(Number)
	const index = year * 12 + (month - 1) + delta
	return `${pad(Math.floor(index / 12), 4)}-${pad((index % 12) + 1)}`
}

/** "November 2026". */
export function periodLabel(period: string, locale?: string): string {
	const [year, month] = period.split('-').map(Number)
	return new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(new Date(year, month - 1, 1))
}

/** A YYYY-MM-DD date as "16 Nov". */
export function dayLabel(date: string, locale?: string): string {
	const [year, month, day] = date.split('-').map(Number)
	return new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(new Date(year, month - 1, day))
}
