import { useMemo, useState } from "react";
import { errorMessage } from "../api";

const COMMON = ["EUR", "USD", "GBP", "CHF"];

function currencyName(code: string): string {
  try {
    return new Intl.DisplayNames(undefined, { type: "currency" }).of(code) ?? code;
  } catch {
    return code;
  }
}

type Props = { initial: string; submitLabel: string; onSubmit: (code: string) => Promise<void> };

export default function CurrencyPicker({ initial, submitLabel, onSubmit }: Props) {
  const [code, setCode] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const all = useMemo(() => {
    const supported = Intl.supportedValuesOf?.("currency") ?? [];
    return [...COMMON, ...supported.filter((c) => !COMMON.includes(c))];
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onSubmit(code);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="row" onSubmit={submit}>
      <label className="field">
        <span>Currency</span>
        <select value={code} onChange={(e) => setCode(e.target.value)}>
          {all.map((c) => (
            <option key={c} value={c}>
              {c} · {currencyName(c)}
            </option>
          ))}
        </select>
      </label>
      <button type="submit" className="primary" disabled={busy}>
        {submitLabel}
      </button>
      {error && <p className="error">{error}</p>}
    </form>
  );
}
