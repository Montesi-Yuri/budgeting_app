import { describe, expect, it } from 'vitest'
import { lessOftenThanMonthly, scheduleLabel, signedAmount } from './planned'

describe('scheduleLabel', () => {
	it('names a monthly schedule by its day', () => {
		expect(scheduleLabel({ kind: 'monthly', day: 27, every_months: 1 }, 'en-GB')).toBe('Monthly on the 27th')
		expect(scheduleLabel({ kind: 'monthly', day: 1, every_months: 1 }, 'en-GB')).toBe('Monthly on the 1st')
		expect(scheduleLabel({ kind: 'monthly', day: 22, every_months: 1 }, 'en-GB')).toBe('Monthly on the 22nd')
		expect(scheduleLabel({ kind: 'monthly', day: 13, every_months: 1 }, 'en-GB')).toBe('Monthly on the 13th')
	})

	it('names every N months', () => {
		expect(scheduleLabel({ kind: 'monthly', day: 3, every_months: 3 }, 'en-GB')).toBe('Every 3 months on the 3rd')
	})

	it('names a yearly schedule by its date', () => {
		expect(scheduleLabel({ kind: 'yearly', month: 6, day: 15 }, 'en-GB')).toBe('Yearly on 15 June')
	})
})

describe('lessOftenThanMonthly', () => {
	it('is true for yearly and every N > 1 months', () => {
		expect(lessOftenThanMonthly({ kind: 'monthly', day: 1, every_months: 1 })).toBe(false)
		expect(lessOftenThanMonthly({ kind: 'monthly', day: 1, every_months: 2 })).toBe(true)
		expect(lessOftenThanMonthly({ kind: 'yearly', month: 6, day: 15 })).toBe(true)
	})
})

describe('signedAmount', () => {
	it('lets the Category decide whether money comes in or goes out', () => {
		expect(signedAmount('income', 250_000)).toBe(250_000)
		expect(signedAmount('expense', 60_000)).toBe(-60_000)
		expect(signedAmount('expense', -60_000)).toBe(-60_000)
	})
})
