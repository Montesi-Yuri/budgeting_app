import { useEffect, useState } from "react";
import { errorMessage, Fulfilment, fulfilmentApi, Occurrence, plannedApi, ReviewList, TransactionView } from "@/shared/api";
import { formatMinor } from "@/shared/lib/money";
import { localToday, periodOf } from "@/shared/lib/periods";
import { Money } from "@/shared/ui";
import { fulfilmentCandidates, linkableOccurrences } from "../lib/fulfilment";
import { sideText } from "../lib/review";

type Props = {
  currency: string;
  review: ReviewList | null;
  transactions: TransactionView[];
  /** Run a change, then refresh the review list. */
  run: (action: () => Promise<unknown>) => void;
};

/** The Planned item parts of the review list: fulfilments and occurrences still expected. */
export default function PlannedReview({ currency, review, transactions, run }: Props) {
  const [fulfilments, setFulfilments] = useState<Fulfilment[]>([]);
  const [thisPeriod, setThisPeriod] = useState<Occurrence[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Reloaded whenever the review list is (after every decision).
  useEffect(() => {
    Promise.all([fulfilmentApi.list(), plannedApi.periodGet(periodOf(localToday()))]).then(
      ([f, plan]) => {
        setFulfilments(f);
        setThisPeriod(plan.occurrences);
        setError(null);
      },
      (e) => setError(errorMessage(e)),
    );
  }, [review]);

  const suggestions = review?.items.flatMap((i) => (i.kind === "fulfilment_suggestion" ? [i.fulfilment] : [])) ?? [];
  const overdue = review?.items.flatMap((i) => (i.kind === "still_expected" ? [i.occurrence] : [])) ?? [];

  return (
    <>
      {error && <p className="error">{error}</p>}
      <section className="card">
        <h2>Planned items that arrived</h2>
        <p className="muted small">
          The actual amount replaces the planned one. Until you decide, a Suggestion counts as fulfilled.
        </p>
        {review && suggestions.length === 0 && <p className="muted small">Nothing to review.</p>}
        <ul className="list">
          {suggestions.map((f) => (
            <li key={f.id} className="row-item">
              <FulfilmentText fulfilment={f} currency={currency} />
              <span className="actions">
                <button className="primary" onClick={() => run(() => fulfilmentApi.confirm(f.id))}>
                  Confirm
                </button>
                <button onClick={() => run(() => fulfilmentApi.unlink(f.id))}>Reject</button>
              </span>
            </li>
          ))}
        </ul>
      </section>

      {overdue.length > 0 && (
        <section className="card">
          <h2>Still expected: skipped or arrives later?</h2>
          <p className="muted small">Their Period has ended and nothing has arrived for them yet.</p>
          <ul className="list">
            {overdue.map((o) => (
              <StillExpected key={o.id} occurrence={o} currency={currency} run={run} />
            ))}
          </ul>
        </section>
      )}

      <LinkForm
        currency={currency}
        occurrences={linkableOccurrences(thisPeriod, overdue)}
        transactions={transactions}
        fulfilments={fulfilments}
        onLink={(o, txnId) => run(() => fulfilmentApi.link(o.planned_item_id, o.scheduled_date, txnId))}
      />

      {fulfilments.length > 0 && (
        <section className="card">
          <h2>Fulfilled Planned items</h2>
          <ul className="list">
            {fulfilments.map((f) => (
              <li key={f.id} className="row-item">
                <FulfilmentText fulfilment={f} currency={currency} />
                <span className="muted small">{f.status === "suggested" ? "to review" : "confirmed"}</span>
                <span className="actions">
                  <button onClick={() => run(() => fulfilmentApi.unlink(f.id))}>Unlink</button>
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}

function FulfilmentText({ fulfilment, currency }: { fulfilment: Fulfilment; currency: string }) {
  return (
    <span className="grow">
      <span className="name">
        {fulfilment.description}, due {fulfilment.due_date}: planned{" "}
        {formatMinor(fulfilment.planned_amount_minor, currency)}
      </span>
      <br />
      <span className="name">Arrived: {sideText(fulfilment.transaction)}</span>{" "}
      <Money minor={fulfilment.transaction.amount_minor} currency={currency} />
    </span>
  );
}

function StillExpected({ occurrence, currency, run }: { occurrence: Occurrence; currency: string } & Pick<Props, "run">) {
  const [newDate, setNewDate] = useState(localToday());
  const moved = occurrence.due_date !== occurrence.scheduled_date;
  return (
    <li className="row-item">
      <span className="grow">
        <span className="name">
          {occurrence.description}, expected {occurrence.due_date}
          {moved && ` (moved from ${occurrence.scheduled_date})`}
        </span>{" "}
        <Money minor={occurrence.amount_minor} currency={currency} />
      </span>
      <span className="actions">
        <button onClick={() => run(() => fulfilmentApi.skip(occurrence.planned_item_id, occurrence.scheduled_date))}>
          Skipped
        </button>
        <input type="date" value={newDate} onChange={(e) => setNewDate(e.target.value)} aria-label="New expected date" />
        <button
          disabled={!newDate}
          onClick={() =>
            run(() => fulfilmentApi.arrivesLater(occurrence.planned_item_id, occurrence.scheduled_date, newDate))
          }
        >
          Arrives later
        </button>
      </span>
    </li>
  );
}

type LinkProps = {
  currency: string;
  occurrences: Occurrence[];
  transactions: TransactionView[];
  fulfilments: Fulfilment[];
  onLink: (occurrence: Occurrence, txnId: string) => void;
};

function LinkForm({ currency, occurrences, transactions, fulfilments, onLink }: LinkProps) {
  const [occurrenceId, setOccurrenceId] = useState("");
  const [txnId, setTxnId] = useState("");
  const chosen = occurrences.find((o) => o.id === occurrenceId) ?? null;
  const candidates = chosen ? fulfilmentCandidates(chosen, transactions, fulfilments) : [];

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!chosen || !txnId) return;
    onLink(chosen, txnId);
    setOccurrenceId("");
    setTxnId("");
  }

  return (
    <form className="card form" onSubmit={submit}>
      <h2>Link to a Planned item by hand</h2>
      <label className="field">
        <span>Expected occurrence</span>
        <select
          value={occurrenceId}
          onChange={(e) => {
            setOccurrenceId(e.target.value);
            setTxnId("");
          }}
        >
          <option value="">Choose an occurrence</option>
          {occurrences.map((o) => (
            <option key={o.id} value={o.id}>
              {o.description} · {o.due_date} · {formatMinor(o.amount_minor, currency)}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span>The Transaction that fulfils it (same Account and direction)</span>
        <select value={txnId} onChange={(e) => setTxnId(e.target.value)} disabled={!chosen}>
          <option value="">Choose a Transaction</option>
          {candidates.map((t) => (
            <option key={t.id} value={t.id}>
              {sideText(t)} · {formatMinor(t.amount_minor, currency)}
            </option>
          ))}
        </select>
      </label>
      <div className="actions">
        <button type="submit" className="primary" disabled={!chosen || !txnId}>
          Link
        </button>
      </div>
    </form>
  );
}
