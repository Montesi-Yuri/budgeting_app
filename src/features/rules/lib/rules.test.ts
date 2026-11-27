import { describe, expect, it } from 'vitest'
import type { RuleView } from '@/shared/api'
import { ruleSummary } from './rules'

const rule = (over: Partial<RuleView> = {}): RuleView => ({
	id: 'r1',
	position: 1,
	match_text: 'coop',
	account_id: null,
	direction: null,
	action: 'categorise',
	category_id: 'c1',
	category_name: 'Groceries',
	account_name: null,
	broken: false,
	...over,
})

describe('ruleSummary', () => {
	it('describes a Rule with no filters', () => {
		expect(ruleSummary(rule())).toBe('Description contains "coop" → Groceries')
	})

	it('names the Account and the direction when the Rule is limited to them', () => {
		expect(ruleSummary(rule({ account_id: 'a1', account_name: 'Card', direction: 'out' }))).toBe(
			'Description contains "coop", on Card, money out → Groceries',
		)
		expect(ruleSummary(rule({ direction: 'in' }))).toBe('Description contains "coop", money in → Groceries')
	})

	it('says when the Category or the Account has been deleted', () => {
		expect(ruleSummary(rule({ broken: true, category_name: null }))).toBe(
			'Description contains "coop" → deleted Category',
		)
		expect(ruleSummary(rule({ account_id: 'a1', account_name: null }))).toBe(
			'Description contains "coop", on a deleted Account → Groceries',
		)
	})
})

describe('ruleSummary for a Transfer Rule', () => {
	it('says it marks the Transaction as a Transfer, and is never shown as a deleted Category', () => {
		expect(ruleSummary(rule({ action: 'transfer', category_id: null, category_name: null }))).toBe(
			'Description contains "coop" → Transfer',
		)
	})
})
