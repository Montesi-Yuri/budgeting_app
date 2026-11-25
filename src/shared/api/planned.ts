import { invoke } from "@tauri-apps/api/core";
import type { CategoryKind } from "./categories";

/**
 * When a Planned item is expected. A day past the end of a short month falls
 * on its last day. every_months counts from the month of the start date.
 */
export type Schedule =
  | { kind: "monthly"; day: number; every_months: number }
  | { kind: "yearly"; month: number; day: number };

export type PlannedItemFields = {
  account_id: string;
  /** Its kind says Income or Expense. */
  category_id: string;
  description: string;
  /** Signed: Income positive, an Expense negative. */
  amount_minor: number;
  schedule: Schedule;
  /** YYYY-MM-DD */
  start_date: string;
  /** YYYY-MM-DD, the last day it can occur; null = no end. */
  end_date: string | null;
};

/** set_aside null = the default (spread when less often than monthly). */
export type NewPlannedItem = PlannedItemFields & { set_aside: boolean | null };

export type PlannedItem = PlannedItemFields & { id: string; set_aside: boolean };

/** An occurrence still expected: calculated from the schedule and the stored decisions. */
export type Occurrence = {
  /** Derived from the Planned item and scheduled_date. */
  id: string;
  planned_item_id: string;
  description: string;
  account_id: string;
  category_id: string;
  kind: CategoryKind;
  /** When it is expected: the schedule's date, or the new date if it arrives later. */
  due_date: string;
  /** The schedule's date, which identifies the occurrence. */
  scheduled_date: string;
  amount_minor: number;
  /** The item is spread as Set-asides, so this occurrence isn't counted again. */
  set_aside: boolean;
};

export type SetAside = {
  planned_item_id: string;
  description: string;
  category_id: string;
  schedule: Schedule;
  item_amount_minor: number;
  /** This Period's share, negative for Expenses. */
  amount_minor: number;
};

export type PeriodPlan = {
  period: string;
  occurrences: Occurrence[];
  set_asides: SetAside[];
  /** Transactions fulfilling items spread as Set-asides, taken back out of Spendable. */
  covered_by_set_asides_minor: number;
};

export const plannedApi = {
  list: () => invoke<PlannedItem[]>("planned_items_list"),
  create: (item: NewPlannedItem) => invoke<PlannedItem>("planned_item_create", { item }),
  update: (item: PlannedItem) => invoke<PlannedItem>("planned_item_update", { item }),
  /** lastDay: the last day the item can occur. */
  end: (id: string, lastDay: string) => invoke<PlannedItem>("planned_item_end", { id, lastDay }),
  delete: (id: string) => invoke<void>("planned_item_delete", { id }),
  /** The expected occurrences and Set-asides of a Period (YYYY-MM). */
  periodGet: (period: string) => invoke<PeriodPlan>("planned_period_get", { period }),
};
