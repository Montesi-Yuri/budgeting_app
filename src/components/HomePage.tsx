import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage, PeriodPlan, PeriodSpendable, plannedApi, SavingsProgress, spendableApi } from "../api";
import { formatMinor } from "../money";
import { dayLabel, localToday, periodLabel, periodOf, shiftPeriod } from "../periods";
import { scheduleLabel } from "../planned";

/** Re-check the date this often, so the Week and days left move at midnight. */
const CLOCK_TICK_MS = 60_000;

export default function HomePage({ currency }: { currency: string }) {
  const [today, setToday] = useState(localToday);
  const current = periodOf(today);
  const [period, setPeriod] = useState(current);
  const [data, setData] = useState<PeriodSpendable | null>(null);
  const [plan, setPlan] = useState<PeriodPlan | null>(null);
  const [savings, setSavings] = useState<SavingsProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    Promise.all([spendableApi.spendableGet(period, today), plannedApi.periodGet(period), api.savingsProgressGet(period)]).then(
      ([d, p, s]) => {
        setData(d);
        setPlan(p);
        setSavings(s);
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
                  <td>Expected Income (Planned items)</td>
                  <td>{money(data.expected_income_minor)}</td>
                </tr>
                <tr>
                  <td>Expected Expenses (Planned items)</td>
                  <td>{money(data.expected_expenses_minor)}</td>
                </tr>
                <tr>
                  <td>Set-asides</td>
                  <td>{money(data.set_asides_minor)}</td>
                </tr>
                {data.covered_by_set_asides_minor !== 0 && (
                  <tr>
                    <td>Paid from Set-asides (already set aside)</td>
                    <td>{money(data.covered_by_set_asides_minor)}</td>
                  </tr>
                )}
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

          {savings && savings.period === period && (
            <section className="card">
              <h2>Savings</h2>
              <table className="breakdown">
                <tbody>
                  <tr>
                    <td>Savings target</td>
                    <td>{money(savings.target_minor)}</td>
                  </tr>
                  <tr>
                    <td>Transferred into savings Accounts {isCurrent ? "so far" : ""}</td>
                    <td>{money(savings.transferred_minor)}</td>
                  </tr>
                </tbody>
              </table>
              {savings.target_minor > 0 && (
                <progress max={savings.target_minor} value={Math.max(0, Math.min(savings.transferred_minor, savings.target_minor))} />
              )}
            </section>
          )}

          {plan && plan.period === period && (
            <section className="card">
              <h2>Planned this Period</h2>
              {plan.occurrences.length === 0 && plan.set_asides.length === 0 && (
                <p className="muted small">Nothing planned. Add Planned items to see expected Income and Expenses here.</p>
              )}
              {plan.occurrences.length > 0 && (
                <table className="breakdown">
                  <tbody>
                    {plan.occurrences.map((o) => (
                      <tr key={`${o.planned_item_id}-${o.due_date}`}>
                        <td>
                          {dayLabel(o.due_date)} · {o.description}{" "}
                          {o.set_aside && <span className="badge muted-badge">covered by its Set-aside</span>}
                        </td>
                        <td>{money(o.amount_minor)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              {plan.set_asides.length > 0 && (
                <>
                  <h2>Set-asides</h2>
                  <table className="breakdown">
                    <tbody>
                      {plan.set_asides.map((a) => (
                        <tr key={a.planned_item_id}>
                          <td>
                            {a.description}{" "}
                            <span className="muted small">
                              {formatMinor(a.item_amount_minor, currency)} {scheduleLabel(a.schedule).toLowerCase()}
                            </span>
                          </td>
                          <td>{money(a.amount_minor)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </>
              )}
            </section>
          )}
        </>
      )}
    </>
  );
}
