import { describe, expect, it } from 'vitest'
import { budgetMeter } from './budgets'

describe('budgetMeter shows how much of a Budget is used', () => {
	it.each([
		[0, 40000, 0, 'ok'],
		[10000, 40000, 25, 'ok'],
		[36000, 40000, 90, 'near'],
		[40000, 40000, 100, 'near'],
		[40001, 40000, 100, 'over'],
		[90000, 40000, 100, 'over'],
		[-1500, 40000, 0, 'ok'], // only Refunds so far
	] as const)('spent %i of %i -> %i%% %s', (spent, amount, percent, tone) => {
		expect(budgetMeter(spent, amount)).toEqual({ percent, tone })
	})
})
