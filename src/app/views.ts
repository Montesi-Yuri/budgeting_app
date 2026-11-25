// The app's views (tabs), and which of them to refetch after a Sync.

export type View = "home" | "forecast" | "review" | "accounts" | "transactions" | "uncategorised" | "categories" | "settings" | "budgets" | "import" | "planned" | "rules";

export const VIEWS: { view: View; label: string }[] = [
  { view: "home", label: "Home" },
  { view: "forecast", label: "Forecast" },
  { view: "review", label: "To review" },
  { view: "accounts", label: "Accounts" },
  { view: "transactions", label: "Transactions" },
  { view: "uncategorised", label: "Uncategorised" },
  { view: "categories", label: "Categories" },
  { view: "settings", label: "Settings" },
  { view: "budgets", label: "Budgets" },
  { view: "import", label: "Import" },
  { view: "planned", label: "Planned" },
  { view: "rules", label: "Rules" },
];

/** The phone's tabs: its main uses are quick add and Spendable, on Home. */
export const PHONE_VIEWS: View[] = ["home", "transactions", "accounts", "categories", "settings"];

/** The views that show synced data. */
// Home refreshes itself on `data://changed`, so it is not listed here.
export type DataView = "forecast" | "review" | "accounts" | "transactions" | "uncategorised" | "categories" | "budgets" | "import" | "planned" | "rules";

const ALL_VIEWS: DataView[] = ["forecast", "review", "accounts", "transactions", "uncategorised", "categories", "budgets", "import", "planned", "rules"];

/** Which views show each synced entity (by `op.entity`, the table name). */
const VIEWS_BY_ENTITY: Record<string, DataView[]> = {
  // Account names and balances appear in every list.
  account: ["forecast", "review", "accounts", "transactions", "uncategorised", "import", "planned", "rules"],
  category: ["forecast", "transactions", "uncategorised", "categories", "budgets", "planned", "rules"],
  txn: ["forecast", "review", "accounts", "transactions", "uncategorised", "budgets"],
  // Transactions in a Transfer leave the Uncategorised list and Budgets.
  transfer: ["forecast", "review", "uncategorised", "budgets"],
  // A match hides its Manual Transaction from balances, Uncategorised, Budgets and the Forecast.
  manual_match: ["forecast", "review", "accounts", "transactions", "uncategorised", "budgets"],
  budget: ["forecast", "budgets"],
  // What is still expected (the review list) follows the Planned items.
  planned_item: ["forecast", "review", "planned"],
  // Fulfilled, skipped or moved occurrences leave or join the review list and the Forecast.
  planned_occurrence: ["forecast", "review"],
  rule: ["rules"],
  import_profile: ["import"],
  import_batch: ["import"],
  // The Currency is reloaded by App itself.
  setting: [],
};

/** The views to refetch after a Sync touched `entities`, in nav order. */
export function viewsToRefresh(entities: string[]): DataView[] {
  const views = new Set<DataView>();
  for (const entity of entities) for (const view of VIEWS_BY_ENTITY[entity] ?? ALL_VIEWS) views.add(view);
  return ALL_VIEWS.filter((v) => views.has(v));
}
