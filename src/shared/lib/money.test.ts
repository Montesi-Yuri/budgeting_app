import { describe, expect, it } from 'vitest'
import { formatMinor, parseAmount, toAmountInput } from '@/shared/lib/money'

// A fixed locale keeps the expected strings independent of the machine.
const LOCALE = 'en-GB'

describe('formatMinor shows integer minor units in the Currency', () => {
	it('formats euro cents', () => {
		expect(formatMinor(125050, 'EUR', LOCALE)).toBe('€1,250.50')
	})

	it('shows credit card debt as negative', () => {
		expect(formatMinor(-35000, 'EUR', LOCALE)).toBe('-€350.00')
	})

	it('uses the Currency\'s own number of decimals', () => {
		expect(formatMinor(1500, 'JPY', LOCALE)).toBe('JP¥1,500')
		expect(formatMinor(1, 'GBP', LOCALE)).toBe('£0.01')
	})

	it('is exact for amounts beyond float precision', () => {
		expect(formatMinor(Number.MAX_SAFE_INTEGER, 'EUR', LOCALE)).toBe('€90,071,992,547,409.91')
	})
})

describe('parseAmount turns typed text into integer minor units', () => {
	it.each([
		['1250.50', 125050],
		['1250,5', 125050],
		['-350', -35000],
		['  12 ', 1200],
		['0.07', 7],
		['1,250.50', 125050],
		['1.250,50', 125050],
	])('%s -> %i cents', (text, minor) => {
		expect(parseAmount(text, 'EUR')).toEqual({ ok: true, minor })
	})

	it('respects a Currency without decimals', () => {
		expect(parseAmount('1500', 'JPY')).toEqual({ ok: true, minor: 1500 })
		expect(parseAmount('15.5', 'JPY').ok).toBe(false)
	})

	it.each(['', 'abc', '1.234', '--5', '1.2.3', '12-'])('rejects %j', (text) => {
		expect(parseAmount(text, 'EUR').ok).toBe(false)
	})
})

describe('toAmountInput writes minor units back as editable text', () => {
	it.each([
		[125050, 'EUR', '1250.50'],
		[-35000, 'EUR', '-350.00'],
		[7, 'EUR', '0.07'],
		[-7, 'EUR', '-0.07'],
		[1500, 'JPY', '1500'],
	])('%i %s -> %s', (minor, currency, text) => {
		expect(toAmountInput(minor, currency)).toBe(text)
		expect(parseAmount(text, currency)).toEqual({ ok: true, minor })
	})
})
