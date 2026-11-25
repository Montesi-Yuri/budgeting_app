// Backup helpers for the UI. A Backup is made and restored by the Rust core
// (budget_core::backup); these only name the file and describe the result.
import type { RestoreReport } from '@/shared/api'

/** The suggested file name of a Backup made on `date` (local day). */
export function backupFileName(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `budget-${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}.bsync.age`
}

function changes(n: number): string {
  return n === 1 ? '1 change' : `${n} changes`
}

/** What to tell the user after a restore. */
export function restoreSummary(report: RestoreReport): string {
  if (report.added === 0) {
    return `Nothing to restore: all ${changes(report.already_present)} in the Backup were already present.`
  }
  return `Backup restored: ${changes(report.added)} added, ${report.already_present} already present.`
}
