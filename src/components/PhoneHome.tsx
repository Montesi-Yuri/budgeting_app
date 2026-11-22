import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage, JoinResult, PeriodSpendable, Peer, spendableApi } from "../api";
import { formatMinor } from "../money";
import { dayLabel, localToday, periodLabel, periodOf } from "../periods";
import { isUnreachable, syncTarget } from "../phone";
import { syncSummary } from "../sync";
import PairWithPc from "./PairWithPc";
import QuickAdd from "./QuickAdd";

const when = (unix: number | null) => (unix ? new Date(unix * 1000).toLocaleString() : "never");

/** The phone's main screen: Spendable, Weekly Spendable, quick add and Sync with the PC. */
export default function PhoneHome({ currency }: { currency: string }) {
  const [data, setData] = useState<PeriodSpendable | null>(null);
  const [peers, setPeers] = useState<Peer[] | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rescan, setRescan] = useState(false);
  // Bumped after a Sync received changes, so quick add reloads the Accounts and Categories.
  const [synced, setSynced] = useState(0);

  const reload = useCallback(() => {
    const today = localToday();
    spendableApi.spendableGet(periodOf(today), today).then(setData, (e) => setError(errorMessage(e)));
  }, []);
  const loadPeers = useCallback(() => api.peersList().then(setPeers, (e) => setError(errorMessage(e))), []);

  useEffect(() => {
    reload();
    void loadPeers();
    const onVisible = () => {
      if (document.visibilityState === "visible") reload();
    };
    document.addEventListener("visibilitychange", onVisible);
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen("data://changed", () => {
      reload();
      setSynced((n) => n + 1);
    })
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch(() => undefined);
    return () => {
      cancelled = true;
      document.removeEventListener("visibilitychange", onVisible);
      unlisten?.();
    };
  }, [reload, loadPeers]);

  const target = peers ? syncTarget(peers) : null;

  async function syncNow() {
    if (!target) return;
    setSyncing(true);
    setError(null);
    setMessage(null);
    setRescan(false);
    try {
      setMessage(syncSummary(await api.syncNow(target.device_id)));
    } catch (e) {
      setError(errorMessage(e));
      setRescan(isUnreachable(e));
    } finally {
      setSyncing(false);
      void loadPeers();
    }
  }

  function joined(j: JoinResult) {
    setError(null);
    setRescan(false);
    setMessage(`Paired with ${j.peer.name}. ${syncSummary(j.report)}`);
    void loadPeers();
  }

  const money = (minor: number) => (
    <span className={minor < 0 ? "amount negative" : "amount"}>{formatMinor(minor, currency)}</span>
  );

  return (
    <div className="phone-home">
      {data && (
        <section className="card phone-spendable">
          <div className="muted small">Spendable · {periodLabel(data.period)}</div>
          <div className="headline">{money(data.spendable_minor)}</div>
          {data.weekly && (
            <div className="phone-weekly">
              <div className="muted small">
                Weekly Spendable · {dayLabel(data.weekly.week_first_day)} – {dayLabel(data.weekly.week_last_day)}
              </div>
              <div className="headline small-headline">{money(data.weekly.amount_minor)}</div>
            </div>
          )}
        </section>
      )}

      <QuickAdd key={synced} currency={currency} onAdded={reload} />

      <section className="card">
        <h2>Sync</h2>
        {peers === null ? null : target ? (
          <>
            <button className="primary big" disabled={syncing} onClick={() => void syncNow()}>
              {syncing ? "Syncing…" : `Sync now with ${target.name}`}
            </button>
            <p className="muted small">Last Sync: {when(target.last_sync_at)}</p>
          </>
        ) : (
          <>
            <p className="muted small">Pair with your PC to Sync. On the PC, open Settings → Sync → Pair new device.</p>
            <PairWithPc mobile onJoined={joined} />
          </>
        )}
        {message && <p className="small">{message}</p>}
        {error && <p className="error small">{error}</p>}
        {rescan && <PairWithPc mobile onJoined={joined} scanLabel="Rescan the PC's QR code (keeps the Pairing)" />}
      </section>
    </div>
  );
}
