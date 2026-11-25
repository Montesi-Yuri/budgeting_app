import { invoke } from '@tauri-apps/api/core'

export type RestoreReport = { added: number, already_present: number }

/** The weekly automatic Backup (PC only). */
export type BackupSchedule = {
  available: boolean
  folder: string | null
  /** File name of the newest weekly Backup in the folder. */
  latest: string | null
  last_error: string | null
}

export const backupApi = {
  export: (path: string, passphrase: string) => invoke<void>('backup_export', { path, passphrase }),
  restore: (path: string, passphrase: string) => invoke<RestoreReport>('backup_restore', { path, passphrase }),
  scheduleGet: () => invoke<BackupSchedule>('backup_schedule_get'),
  scheduleSet: (folder: string | null, passphrase: string | null) =>
    invoke<BackupSchedule>('backup_schedule_set', { folder, passphrase }),
}
