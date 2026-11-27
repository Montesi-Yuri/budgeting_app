import { describe, expect, it } from 'vitest'
import type { Category } from '@/shared/api'
import { categoryChoices, siblingPlace, transactionFlow } from './categories'

const cat = (id: string, name: string, kind: Category['kind'], parent_id: string | null = null): Category => ({
	id,
	name,
	kind,
	parent_id,
	color: null,
	position: 1,
})

// As categories_list returns them: each parent followed by its children.
const list = [
	cat('food', 'Food', 'expense'),
	cat('groceries', 'Groceries', 'expense', 'food'),
	cat('restaurants', 'Restaurants', 'expense', 'food'),
	cat('salary', 'Salary', 'income'),
	cat('home', 'Home', 'expense'),
]

describe('transactionFlow: Income or Expense is decided by the Category, not the sign', () => {
	it.each([
		['expense', -500, 'expense'],
		['expense', 500, 'refund'],
		['income', 500, 'income'],
		['income', -500, 'income'],
		[null, -500, 'uncategorised'],
		[null, 500, 'uncategorised'],
	] as const)('%s Category, amount %i -> %s', (kind, amount, flow) => {
		expect(transactionFlow(kind, amount)).toBe(flow)
	})
})

describe('categoryChoices groups the Categories for a picker', () => {
	it('puts expense Categories first, keeps tree order and names children with their parent', () => {
		expect(categoryChoices(list)).toEqual([
			{
				kind: 'expense',
				label: 'Expense',
				options: [
					{ id: 'food', label: 'Food' },
					{ id: 'groceries', label: 'Food › Groceries' },
					{ id: 'restaurants', label: 'Food › Restaurants' },
					{ id: 'home', label: 'Home' },
				],
			},
			{ kind: 'income', label: 'Income', options: [{ id: 'salary', label: 'Salary' }] },
		])
	})

	it('leaves out a kind with no Categories', () => {
		expect(categoryChoices([cat('salary', 'Salary', 'income')]).map((g) => g.kind)).toEqual(['income'])
	})
})

describe('siblingPlace tells where a Category sits among its siblings', () => {
	it.each([
		['food', 0, 3],
		['home', 2, 3],
		['salary', 1, 3],
		['restaurants', 1, 2],
	])('%s is %i of %i', (id, index, count) => {
		expect(siblingPlace(list, id)).toEqual({ index, count })
	})
})
