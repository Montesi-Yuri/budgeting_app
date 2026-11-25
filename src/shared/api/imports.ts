import { invoke } from '@tauri-apps/api/core'

export type ImportEncoding = 'utf-8' | 'windows-1252'
export type AmountMode = 'signed' | 'split'

/** One Account's saved CSV layout. Columns are named by their header text. */
export type ImportProfile = {
  id: string
  account_id: string
  name: string
  encoding: ImportEncoding
  /** One character: ";", "," or a tab. */
  delimiter: string
  /** Lines before the header line. */
  skip_rows: number
  date_column: string
  /** strftime style, e.g. "%d/%m/%Y". */
  date_format: string
  amount_mode: AmountMode
  amount_column: string | null
  debit_column: string | null
  credit_column: string | null
  decimal_sep: ',' | '.'
  thousands_sep: string | null
  /** Joined with a space, in this order. */
  description_columns: string[]
  balance_column: string | null
}

export type PreviewRow = {
  line: number
  date: string | null
  amount_minor: number | null
  description: string
  balance_after_minor: number | null
  /** Why this line can't be imported. */
  error: string | null
}

export type ImportPreview = {
  first_lines: string[]
  columns: string[]
  /** Stops the profile from reading the file at all; rows is then empty. */
  problem: string | null
  rows: PreviewRow[]
  total_rows: number
  rows_with_errors: number
}

/** One Import in an Account's history. */
export type ImportBatch = {
  id: string
  account_id: string
  profile_id: string
  source_name: string
  /** Unix ms. */
  imported_at: number
  first_date: string | null
  last_date: string | null
  rows_new: number
  rows_skipped: number
}

export type BalanceMismatch = {
  line: number
  date: string
  description: string
  file_balance_minor: number
  calculated_balance_minor: number
}

export type ImportSummary = { batch: ImportBatch, balance_mismatch: BalanceMismatch | null }

export const importsApi = {
  profileGet: (accountId: string) => invoke<ImportProfile | null>('import_profile_get', { account: accountId }),
  profileDraft: (accountId: string) => invoke<ImportProfile>('import_profile_draft', { account: accountId }),
  profileSave: (profile: ImportProfile) => invoke<ImportProfile>('import_profile_save', { profile }),
  /** Read the file's bytes with a profile; nothing is saved. */
  preview: (profile: ImportProfile, file: Uint8Array) =>
    invoke<ImportPreview>('import_preview', { profile, file: Array.from(file) }),
  /** Import with the Account's saved profile. The file itself is not kept. */
  run: (accountId: string, fileName: string, file: Uint8Array) =>
    invoke<ImportSummary>('import_run', { account: accountId, fileName, file: Array.from(file) }),
  history: (accountId: string) => invoke<ImportBatch[]>('imports_history', { account: accountId }),
}
