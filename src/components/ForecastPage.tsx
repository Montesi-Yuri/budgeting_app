import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { errorMessage, Forecast, forecastApi, ForecastPoint } from "../api";
import { confidenceText, netBars } from "../forecast";
import { formatMinor } from "../money";
import { localToday, periodLabel } from "../periods";

export default function ForecastPage({ currency }: { currency: string }) {
  const [forecast, setForecast] = useState<Forecast | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    forecastApi.get(localToday()).then(
      (f) => {
        setForecast(f);
        setError(null);
      },
      (e) => setError(errorMessage(e)),
    );
  }, []);
  useEffect(reload, [reload]);

  // Recalculate on returning to the app and when a Sync applied changes.
  useEffect(() => {
    const onVisible = () => {
      if (document.visibilityState === "visible") reload();
    };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("data://changed", reload)
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch(() => undefined);
    return () => {
      cancelled = true;
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
      unlisten?.();
    };
  }, [reload]);

  const money = (minor: number, className = "amount") => (
    <span className={minor < 0 ? `${className} negative` : className}>{formatMinor(minor, currency)}</span>
  );

  if (error) return <p className="error">{error}</p>;
  if (!forecast) return <p className="muted">Loading…</p>;

  const lowConfidence = confidenceText(forecast.complete_periods);
  const endOfPeriodLabel = `End of ${periodLabel(forecast.end_of_period.date.slice(0, 7))}`;
  const endOfYearLabel = `End of ${forecast.end_of_year.date.slice(0, 4)}`;
  const bars = netBars(forecast.months.map((m) => m.net_position_minor));

  const projection = (label: string, point: ForecastPoint) => (
    <div className="forecast-point">
      <div className="muted small">{label}</div>
      <div className="headline small-headline">{money(point.net_position_minor)}</div>
      {forecast.main_account_name && point.main_account_minor !== null && (
        <div className="muted small">
          {forecast.main_account_name}: {money(point.main_account_minor)}
        </div>
      )}
    </div>
  );

  return (
    <>
      <h1>Forecast</h1>
      {lowConfidence && (
        <p className="card low-confidence" role="status">
          {lowConfidence} Everyday spending without a Budget is estimated from the history there is.
        </p>
      )}

      <section className="card forecast-points" aria-label="Projected Net position">
        {projection("Net position today", forecast.today)}
        {projection(endOfPeriodLabel, forecast.end_of_period)}
        {projection(endOfYearLabel, forecast.end_of_year)}
      </section>

      <section className="card">
        <h2>Month by month</h2>
        <table className="breakdown forecast-months">
          <caption className="muted small">Projected Net position at the end of each Period</caption>
          <thead>
            <tr>
              <th scope="col">Period</th>
              <th scope="col">Planned items</th>
              <th scope="col">Everyday spending</th>
              <th scope="col">Net position</th>
            </tr>
          </thead>
          <tbody>
            {forecast.months.map((m, i) => (
              <tr key={m.period}>
                <th scope="row">
                  {periodLabel(m.period)}
                  <div className="forecast-bar" aria-hidden="true">
                    <span className={bars[i].negative ? "negative" : ""} style={{ width: `${bars[i].percent}%` }} />
                  </div>
                </th>
                <td>{money(m.planned_minor)}</td>
                <td>{money(-m.everyday_minor)}</td>
                <td>{money(m.net_position_minor)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="muted small">
          Planned items are counted on their due dates, a yearly one in its own month. Transfers are not projected: they
          move money between Accounts without changing the Net position.
        </p>
      </section>

      <section className="card">
        <h2>Everyday spending this Period</h2>
        {forecast.everyday.length === 0 ? (
          <p className="muted small">
            No everyday spending expected yet. Add Budgets, or import a few Periods of Transactions.
          </p>
        ) : (
          <table className="breakdown">
            <thead>
              <tr>
                <th scope="col">Category</th>
                <th scope="col">Per Period</th>
                <th scope="col">Still expected</th>
              </tr>
            </thead>
            <tbody>
              {forecast.everyday.map((e) => (
                <tr key={e.category_id ?? "uncategorised"}>
                  <td>
                    {e.name}{" "}
                    <span className="badge muted-badge">{e.source === "budget" ? "Budget" : "average of last 3 Periods"}</span>
                  </td>
                  <td>{money(-e.per_period_minor)}</td>
                  <td>{money(-e.left_this_period_minor)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="muted small">
          Categories with a Planned item are left out, so they aren't counted twice. In later Periods the whole amount is
          expected.
        </p>
      </section>
    </>
  );
}
