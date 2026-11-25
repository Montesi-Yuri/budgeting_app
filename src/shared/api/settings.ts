import { invoke } from '@tauri-apps/api/core'

export type AppInfo = {
  device_id: string
  schema_version: number
  /** "windows", "android", ... */
  platform: string
  /** The phone build: phone layout and the QR code scanner. */
  mobile: boolean
}

export const settingsApi = {
  appInfo: () => invoke<AppInfo>('app_info'),
  currencyGet: () => invoke<string | null>('currency_get'),
  currencySet: (code: string) => invoke<void>('currency_set', { code }),
}
