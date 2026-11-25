import { describe, expect, it } from 'vitest'
import { dayLabel, localToday, periodLabel, periodOf, shiftPeriod } from '@/shared/lib/periods'

describe('Periods are calendar months written YYYY-MM', () => {
  it('today is the local calendar date', () => {
    expect(localToday(new Date(2026, 10, 21, 23, 59))).toBe('2026-11-21')
    expect(localToday(new Date(2027, 0, 5, 0, 1))).toBe('2027-01-05')
  })

  it('finds the Period of a date', () => {
    expect(periodOf('2026-11-21')).toBe('2026-11')
  })

  it('steps back and forward across the year boundary', () => {
    expect(shiftPeriod('2027-01', -1)).toBe('2026-12')
    expect(shiftPeriod('2026-12', 1)).toBe('2027-01')
    expect(shiftPeriod('2026-11', -13)).toBe('2025-10')
  })

  it('names a Period by month and year', () => {
    expect(periodLabel('2026-11', 'en-GB')).toBe('November 2026')
  })
})

describe('dayLabel', () => {
  it('names a day of a Week by day and month', () => {
    expect(dayLabel('2026-11-16', 'en-GB')).toBe('16 Nov')
    expect(dayLabel('2026-12-01', 'en-GB')).toBe('1 Dec')
  })
})
