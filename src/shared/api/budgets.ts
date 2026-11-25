import { invoke } from '@tauri-apps/api/core'

export type BudgetPer = 'period' | 'week'

export type NewBudget = {
  /** An expense Category; a parent covers its children. */
  category_id: string
  per: BudgetPer
  /** The limit per Period or Week, positive minor units. */
  amount_minor: number
  /** YYYY-MM-DD: the first day it applies. */
  valid_from: string
}

export type BudgetEdit = NewBudget & { id: string }

/** valid_to: the last day it applies, null until it is ended. */
export type Budget = BudgetEdit & { valid_to: string | null }

/** broken: its Category was deleted; it is not applied until another is picked. */
export type BudgetState = 'active' | 'upcoming' | 'ended' | 'broken'

export type BudgetProgress = {
  budget: Budget
  state: BudgetState
  /** The current Period or Week; null unless active. */
  range: { from: string, to: string } | null
  /** Spending in range, reduced by Refunds. */
  spent_minor: number
  /** Negative when over. */
  left_minor: number
  over: boolean
}

export const budgetsApi = {
  /** Every Budget with its progress on `today` (YYYY-MM-DD). */
  progress: (today: string) => invoke<BudgetProgress[]>('budgets_progress', { today }),
  create: (budget: NewBudget) => invoke<Budget>('budget_create', { budget }),
  update: (budget: BudgetEdit) => invoke<Budget>('budget_update', { budget }),
  /** lastDay: the last day the Budget applies. */
  end: (id: string, lastDay: string) => invoke<Budget>('budget_end', { id, lastDay }),
  delete: (id: string) => invoke<void>('budget_delete', { id }),
}
