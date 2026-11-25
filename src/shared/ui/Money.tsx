import { formatMinor } from "@/shared/lib/money";

/** An amount in the Currency, styled red when negative. */
export default function Money({ minor, currency }: { minor: number; currency: string }) {
  return <span className={minor < 0 ? "amount negative" : "amount"}>{formatMinor(minor, currency)}</span>;
}
