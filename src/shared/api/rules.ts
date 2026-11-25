import { invoke } from '@tauri-apps/api/core'
import type { TransactionView } from './transactions'

export type RuleDirection = 'in' | 'out'

/** categorise: assign the Category. transfer: mark as part of a Transfer (no Category). */
export type RuleAction = 'categorise' | 'transfer'

/** What the user enters for a Rule. match_text is looked for in the description, ignoring case. */
export type NewRule = {
  match_text: string
  /** null = any Account. */
  account_id: string | null
  /** null = money in or out. */
  direction: RuleDirection | null
  action: RuleAction
  /** Required to categorise; null for a Transfer Rule. */
  category_id: string | null
}

export type RuleEdit = NewRule & { id: string }

export type Rule = RuleEdit & {
  /** First match wins, lowest first. */
  position: number
}

/** broken: its Category was deleted; it is not applied until another is picked. */
export type RuleView = Rule & { category_name: string | null, account_name: string | null, broken: boolean }

export const rulesApi = {
  /** Every Rule, in the order they are tried. */
  list: () => invoke<RuleView[]>('rules_list'),
  create: (rule: NewRule) => invoke<Rule>('rule_create', { rule }),
  update: (rule: RuleEdit) => invoke<Rule>('rule_update', { rule }),
  delete: (id: string) => invoke<void>('rule_delete', { id }),
  /** Move to `index` in the order (0 = tried first). Only this Rule is written. */
  move: (id: string, index: number) => invoke<Rule>('rule_move', { id, index }),
  /** The existing Transactions the Rule would re-categorise; nothing is changed. */
  recategorisePreview: (id: string) => invoke<TransactionView[]>('rule_recategorise_preview', { id }),
  /** Apply the Rule to the confirmed Transactions. Returns how many changed. */
  recategorise: (id: string, transactionIds: string[]) =>
    invoke<number>('rule_recategorise', { id, transactions: transactionIds }),
}
