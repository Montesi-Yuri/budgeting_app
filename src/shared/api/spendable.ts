import { invoke } from '@tauri-apps/api/core'

/** The Weekly Spendable for today's Week (cut at the Period boundary). */
export type WeeklySpendable = {
	week_first_day: string
	week_last_day: string
	/** Counting today. */
	days_left_in_period: number
	/** Counting today. */
	days_left_in_week: number
	amount_minor: number
}

/** Spendable for one Period (YYYY-MM) and the terms it is made of. */
export type PeriodSpendable = {
	period: string
	first_day: string
	last_day: string
	income_minor: number
	/** Negative; Refunds included. */
	expenses_minor: number
	/** Counted as Expenses (negative) or Refunds (positive). */
	uncategorised_minor: number
	/** Expected Income occurrences of Planned items not spread as Set-asides. */
	expected_income_minor: number
	/** Expected Expense occurrences of Planned items not spread as Set-asides (negative). */
	expected_expenses_minor: number
	/** The Period's Set-asides, negative for Expenses. */
	set_asides_minor: number
	/** Transactions fulfilling items spread as Set-asides, taken back out (positive for Expenses). */
	covered_by_set_asides_minor: number
	savings_target_minor: number
	spendable_minor: number
	/** Only when today is in this Period. */
	weekly: WeeklySpendable | null
}

/** The Savings target next to what Transfers moved into savings Accounts in a Period. */
export type SavingsProgress = { period: string, target_minor: number, transferred_minor: number }

export const spendableApi = {
	savingsTargetGet: () => invoke<number>('savings_target_get'),
	savingsTargetSet: (minor: number) => invoke<void>('savings_target_set', { minor }),
	/** Savings progress for a Period (YYYY-MM). */
	savingsProgressGet: (period: string) => invoke<SavingsProgress>('savings_progress_get', { period }),
	/** `today` is the local date (YYYY-MM-DD). */
	spendableGet: (period: string, today: string) => invoke<PeriodSpendable>('spendable_get', { period, today }),
}
