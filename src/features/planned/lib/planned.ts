// Pure helpers for showing and entering Planned items (no I/O). Expected
// occurrences and Set-asides are calculated in budget_core.
import type { CategoryKind, Schedule } from '@/shared/api'

function ordinal(n: number): string {
  const tens = n % 100
  if (tens >= 11 && tens <= 13) return `${n}th`
  return `${n}${{ 1: 'st', 2: 'nd', 3: 'rd' }[n % 10] ?? 'th'}`
}

/** "Monthly on the 27th", "Every 3 months on the 1st", "Yearly on 15 June". */
export function scheduleLabel(schedule: Schedule, locale?: string): string {
  if (schedule.kind === 'yearly') {
    const date = new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'long' }).format(
      new Date(2024, schedule.month - 1, schedule.day),
    )
    return `Yearly on ${date}`
  }
  const every = schedule.every_months === 1 ? 'Monthly' : `Every ${schedule.every_months} months`
  return `${every} on the ${ordinal(schedule.day)}`
}

/** Items less frequent than monthly are spread as Set-asides by default. */
export function lessOftenThanMonthly(schedule: Schedule): boolean {
  return schedule.kind === 'yearly' || schedule.every_months > 1
}

/** The amount as it will appear on the Account: the Category decides the sign. */
export function signedAmount(kind: CategoryKind, minor: number): number {
  const size = Math.abs(minor)
  return kind === 'income' ? size : -size
}
