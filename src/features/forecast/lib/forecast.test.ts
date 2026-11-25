import { describe, expect, it } from 'vitest'
import { confidenceText, netBars } from './forecast'

describe('netBars scales the month-by-month Net position', () => {
  it('makes the largest amount a full bar and the others relative to it', () => {
    expect(netBars([40000, 20000, 10000])).toEqual([
      { percent: 100, negative: false },
      { percent: 50, negative: false },
      { percent: 25, negative: false },
    ])
  })

  it('measures a negative Net position by its size and marks it', () => {
    expect(netBars([10000, -20000])).toEqual([
      { percent: 50, negative: false },
      { percent: 100, negative: true },
    ])
  })

  it('shows no bars when everything is zero', () => {
    expect(netBars([0, 0])).toEqual([
      { percent: 0, negative: false },
      { percent: 0, negative: false },
    ])
  })

  it('keeps a tiny amount visible', () => {
    expect(netBars([100000, 10])[1].percent).toBe(1)
  })
})

describe('confidenceText says how much history the Forecast is based on', () => {
  it.each([
    [0, 'Low confidence: no complete Period of history yet.'],
    [1, 'Low confidence: only 1 complete Period of history.'],
    [2, 'Low confidence: only 2 complete Periods of history.'],
  ])('%i complete Periods', (n, text) => {
    expect(confidenceText(n)).toBe(text)
  })

  it('says nothing with 3 or more complete Periods', () => {
    expect(confidenceText(3)).toBeNull()
    expect(confidenceText(12)).toBeNull()
  })
})
