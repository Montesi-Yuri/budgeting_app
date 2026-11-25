import { invoke } from '@tauri-apps/api/core'

/** Projected balances on one day. */
export type ForecastPoint = {
  date: string
  /** All Accounts, credit card debt negative. */
  net_position_minor: number
  /** Null when no Account is marked as main. */
  main_account_minor: number | null
}

/** One Period of the Forecast, from the current one to December. */
export type ForecastMonth = {
  period: string
  /** Expected Planned item occurrences after today, signed. */
  planned_minor: number
  /** Estimated everyday spending after today (positive). */
  everyday_minor: number
  net_position_minor: number
  main_account_minor: number | null
}

export type EverydayEstimate = {
  /** Null for the Uncategorised Transactions. */
  category_id: string | null
  name: string
  source: 'budget' | 'average'
  per_period_minor: number
  left_this_period_minor: number
}

export type Forecast = {
  today: ForecastPoint
  end_of_period: ForecastPoint
  end_of_year: ForecastPoint
  main_account_name: string | null
  months: ForecastMonth[]
  everyday: EverydayEstimate[]
  complete_periods: number
  low_confidence: boolean
}

export const forecastApi = {
  /** `today` is the local date (YYYY-MM-DD). */
  get: (today: string) => invoke<Forecast>('forecast_get', { today }),
}
