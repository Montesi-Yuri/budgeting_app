// Pure helpers for showing Rules (no I/O).
import type { RuleView } from "./api";

/** One line saying what a Rule looks for and what it assigns. */
export function ruleSummary(rule: RuleView): string {
  const parts = [`Description contains "${rule.match_text}"`];
  if (rule.account_id !== null) parts.push(`on ${rule.account_name ?? "a deleted Account"}`);
  if (rule.direction !== null) parts.push(rule.direction === "in" ? "money in" : "money out");
  const target = rule.broken || rule.category_name === null ? "deleted Category" : rule.category_name;
  return `${parts.join(", ")} → ${target}`;
}
