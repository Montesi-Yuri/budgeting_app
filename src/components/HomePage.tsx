import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { errorMessage, PeriodSpendable, spendableApi } from "../api";
import { formatMinor } from "../money";
import { dayLabel, localToday, periodLabel, periodOf, shiftPeriod } from "../periods";

/** Re-check the date this often, so the Week and days left move at midnight. */
const CLOCK_TICK_MS = 60_000;

export default function HomePage({ currency }: { currency: string }) {
  const [today, setToday] = useState(localToday);
  const current = periodOf(today);
  const [period, setPeriod] = useState(current);
  const [data, setData] = useState<PeriodSpendable | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    spendableApi.spendableGet(period, today).then(
      (d) => {
        setData(d);
        setError(null);
      },
      (e) => setError(errorMessage(e)),
    );
  }, [period, today]);
  useEffect(reload, [reload]);

  // Recalculate whenever data may have changed: on returning to the app, and
  // when a Sync batch is applied (the shell emits "data://changed").
  useEffect(() => {
    const tick = () => setToday(localToday());
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        tick();
        reload();
      }
    };
    const timer = window.setInterval(tick, CLOCK_TICK_MS);
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("data://changed", reload)
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch(() => undefined);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
      unlisten?.();
    };
  }, [reload]);

  // When the date moves into a new Period, follow it if the user was on the current one.
  const [shownCurrent, setShownCurrent] = useState(current);
  if (shownCurrent !== current) {
    setShownCurrent(current);
    if (period === shownCurrent) setPeriod(current);
  }

  const money = (minor: number, className = "amount") => (
    <span className={minor < 0 ? `${className} negative` : className}>{formatMinor(minor, currency)}</span>
  );
  const isCurrent = period === current;

  return (
    <>
      <div className="heading">
        <h1>{periodLabel(period)}</h1>
        <div className="actions">
          <button onClick={() => setPeriod(shiftPeriod(period, -1))} aria-label="Previous Period">
            ‹ Previous
          </button>
          {!isCurrent && <button onClick={() => setPeriod(current)}>This Period</button>}
          <button onClick={() => setPeriod(shiftPeriod(period, 1))} disabled={isCurrent} aria-label="Next Period">
            Next ›
          </button>
        </div>
      </div>
      {error && <p className="error">{error}</p>}

      {data && data.period === period && (
        <>
          <section className="card spendable">
            <div>
              <div className="muted small">{isCurrent ? "Spendable this Period" : "Spendable at the end of the Period"}</div>
              <div className="headline">{money(data.spendable_minor)}</div>
            </div>
            {data.weekly && (
              <div className="weekly">
                <div className="muted small">
                  Weekly Spendable · {dayLabel(data.weekly.week_first_day)} – {dayLabel(data.weekly.week_last_day)}
                </div>
                <div className="headline small-headline">{money(data.weekly.amount_minor)}</div>
                <div className="muted small">
                  {data.weekly.days_left_in_week} of the {data.weekly.days_left_in_period} days left in the Period
                </div>
              </div>
            )}
          </section>

          <section className="card">
            <h2>How it is calculated</h2>
            <table className="breakdown">
              <tbody>
                <tr>
                  <td>Income</td>
                  <td>{money(data.income_minor)}</td>
                </tr>
                <tr>
                  <td>Expenses (Refunds included)</td>
                  <td>{money(data.expenses_minor)}</td>
                </tr>
                <tr>
                  <td>Uncategorised</td>
                  <td>{money(data.uncategorised_minor)}</td>
                </tr>
                <tr>
                  <td>Savings target</td>
                  <td>{money(-data.savings_target_minor)}</td>
                </tr>
                <tr className="total">
                  <td>Spendable</td>
                  <td>{money(data.spendable_minor)}</td>
                </tr>
              </tbody>
            </table>
            <p className="muted small">
              Each Period starts fresh. The Savings target is set in Settings.
            </p>
          </section>
        </>
      )}
    </>
  );
}
