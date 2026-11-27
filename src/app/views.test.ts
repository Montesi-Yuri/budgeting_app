import { describe, expect, it } from 'vitest'
import { viewsToRefresh } from './views'

describe('viewsToRefresh: the views showing what a Sync changed', () => {
	it('refreshes every view that shows an Account after Accounts changed', () => {
		expect(viewsToRefresh(['account'])).toEqual(['forecast', 'review', 'accounts', 'transactions', 'uncategorised', 'import', 'planned', 'rules'])
	})
	it('refreshes Categories and the lists showing their names after Categories changed', () => {
		expect(viewsToRefresh(['category'])).toEqual(['forecast', 'transactions', 'uncategorised', 'categories', 'budgets', 'planned', 'rules'])
	})
	it('refreshes the lists and the balances after Transactions changed', () => {
		expect(viewsToRefresh(['txn'])).toEqual(['forecast', 'review', 'accounts', 'transactions', 'uncategorised', 'budgets'])
	})
	it('refreshes everything for an entity it doesn\'t know yet', () => {
		expect(viewsToRefresh(['some_future_entity'])).toEqual(['forecast', 'review', 'accounts', 'transactions', 'uncategorised', 'categories', 'budgets', 'import', 'planned', 'rules'])
	})
	it('refreshes Budgets after Budgets changed and Import after Import profiles changed', () => {
		expect(viewsToRefresh(['budget'])).toEqual(['forecast', 'budgets'])
		expect(viewsToRefresh(['rule'])).toEqual(['rules'])
		expect(viewsToRefresh(['import_profile', 'import_batch'])).toEqual(['import'])
	})
	it('refreshes the review list, the Uncategorised list and Budgets after Transfers changed', () => {
		expect(viewsToRefresh(['transfer'])).toEqual(['forecast', 'review', 'uncategorised', 'budgets'])
	})
	it('refreshes the review list after Planned item occurrences or Planned items changed', () => {
		expect(viewsToRefresh(['planned_occurrence'])).toEqual(['forecast', 'review'])
		expect(viewsToRefresh(['planned_item'])).toEqual(['forecast', 'review', 'planned'])
	})
	it('refreshes the review list and every view with numbers after a match with a Manual Transaction changed', () => {
		expect(viewsToRefresh(['manual_match'])).toEqual(['forecast', 'review', 'accounts', 'transactions', 'uncategorised', 'budgets'])
	})
	it('refreshes nothing when nothing changed', () => {
		expect(viewsToRefresh([])).toEqual([])
	})
})
