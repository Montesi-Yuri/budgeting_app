// Pure helpers for the Import screens: first guesses for a new Import
// profile, and the Import summary text. Parsing itself happens in Rust.
import type { ImportProfile } from "@/shared/api";

const DELIMITERS = [";", ",", "\t"] as const;

/** The separator used most often outside quotes in a header line. */
export function guessDelimiter(line: string): (typeof DELIMITERS)[number] {
  const counts = new Map<string, number>(DELIMITERS.map((d) => [d, 0]));
  let quoted = false;
  for (const ch of line) {
    if (ch === '"') quoted = !quoted;
    else if (!quoted && counts.has(ch)) counts.set(ch, counts.get(ch)! + 1);
  }
  return DELIMITERS.reduce((best, d) => (counts.get(d)! > counts.get(best)! ? d : best), DELIMITERS[0]);
}

// Header names banks commonly use, lower case; the first column matching is chosen.
const NAMES = {
  date: ["data contabile", "data operazione", "data", "date", "booking date", "transaction date"],
  amount: ["importo", "amount", "importo eur", "betrag"],
  debit: ["dare", "addebiti", "uscite", "debit", "debits", "withdrawal", "money out"],
  credit: ["avere", "accrediti", "entrate", "credit", "credits", "deposit", "money in"],
  description: ["descrizione", "descrizione operazione", "description", "payee", "causale", "memo", "details"],
  balance: ["saldo", "balance", "running balance"],
};

function find(columns: string[], names: string[]): string | null {
  for (const name of names) {
    const hit = columns.find((c) => c.trim().toLowerCase() === name);
    if (hit) return hit;
  }
  return null;
}

/** Fill the column choices that are still empty with recognised header names. */
export function guessColumns(profile: ImportProfile, columns: string[]): ImportProfile {
  const next = { ...profile };
  if (!next.date_column) next.date_column = find(columns, NAMES.date) ?? "";
  const noAmountChosen = !next.amount_column && !next.debit_column && !next.credit_column;
  if (noAmountChosen) {
    const amount = find(columns, NAMES.amount);
    const debit = find(columns, NAMES.debit);
    const credit = find(columns, NAMES.credit);
    if (amount) {
      next.amount_mode = "signed";
      next.amount_column = amount;
    } else if (debit && credit) {
      next.amount_mode = "split";
      next.debit_column = debit;
      next.credit_column = credit;
    }
  }
  if (next.description_columns.length === 0) {
    const description = find(columns, NAMES.description);
    if (description) next.description_columns = [description];
  }
  if (!next.balance_column) next.balance_column = find(columns, NAMES.balance);
  return next;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "1 new Transaction, 2 skipped because they were already imported." */
export function importResultText(rowsNew: number, rowsSkipped: number): string {
  const created = rowsNew === 0 ? "No new Transactions" : plural(rowsNew, "new Transaction", "new Transactions");
  if (rowsSkipped === 0) return `${created}.`;
  const they = rowsSkipped === 1 ? "it was" : "they were";
  return `${created}, ${rowsSkipped} skipped because ${they} already imported.`;
}
