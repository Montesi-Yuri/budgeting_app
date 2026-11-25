import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { errorMessage, JoinResult, PairingInfo, Peer, SyncProgress, SyncReport, SyncStatus, syncApi } from "@/shared/api";
import { isUnreachable } from "../lib/pairing";
import { expiresIn, needsFirewallExplanation, progressText, syncSummary } from "../lib/sync";
import PairWithPc from "./PairWithPc";
import PairingQr from "./PairingQr";

const when = (unix: number | null) => (unix ? new Date(unix * 1000).toLocaleString() : "never");

/** Sync on/off, Pairing and the paired Devices (Settings page). On the phone, joining scans the PC's QR code. */
export default function SyncSettings({ mobile = false }: { mobile?: boolean }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [port, setPort] = useState("");
  const [peers, setPeers] = useState<Peer[]>([]);
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [now, setNow] = useState(Date.now());
  const [rescan, setRescan] = useState(false);
  const [explainFirewall, setExplainFirewall] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPeers = () => syncApi.peers().then(setPeers, (e) => setError(errorMessage(e)));

  useEffect(() => {
    syncApi.status().then((s) => {
      setStatus(s);
      setPort(String(s.port));
    }, (e) => setError(errorMessage(e)));
    void loadPeers();
    const unlisten = [
      listen<SyncProgress>("sync://progress", (e) => setProgress(e.payload)),
      listen<Peer>("sync://paired", (e) => {
        setPairing(null);
        setMessage(`Paired with ${e.payload.name}. Running the first Sync…`);
        void loadPeers();
      }),
      listen<SyncReport>("sync://finished", (e) => {
        setProgress(null);
        setMessage(syncSummary(e.payload));
        void loadPeers();
      }),
      listen<string>("sync://failed", (e) => {
        setProgress(null);
        setError(e.payload);
      }),
    ];
    return () => unlisten.forEach((p) => void p.then((off) => off()));
  }, []);

  useEffect(() => {
    if (!pairing) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [pairing]);

  async function run<T>(label: string, action: () => Promise<T>): Promise<T | undefined> {
    setBusy(label);
    setError(null);
    setMessage(null);
    setRescan(false);
    try {
      return await action();
    } catch (e) {
      setError(errorMessage(e));
      setRescan(isUnreachable(e));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  }

  /** `firewall`: the answer to the firewall explanation, shown first the first time Sync is turned on. */
  async function setServer(enabled: boolean, firewall?: boolean) {
    const portNumber = Number(port);
    if (!Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65535) {
      setError("Choose a port between 1 and 65535.");
      return;
    }
    if (firewall === undefined && status && needsFirewallExplanation(status, enabled)) {
      setExplainFirewall(true);
      return;
    }
    const next = await run("server", () => syncApi.serverSet(enabled, portNumber, firewall));
    if (next) {
      setStatus(next);
      setExplainFirewall(false);
    }
    if (!enabled) setPairing(null);
  }

  async function startPairing() {
    const info = await run("pairing", syncApi.pairingStart);
    if (info) {
      setNow(Date.now());
      setPairing(info);
    }
  }

  async function cancelPairing() {
    setPairing(null);
    await syncApi.pairingCancel();
  }

  function joined(joined: JoinResult) {
    setError(null);
    setRescan(false);
    setMessage(`Paired with ${joined.peer.name}. ${syncSummary(joined.report)}`);
    void loadPeers();
  }

  async function syncNow(peer: Peer) {
    const report = await run(`sync:${peer.device_id}`, () => syncApi.now(peer.device_id));
    if (report) {
      setMessage(`${peer.name}: ${syncSummary(report)}`);
      void loadPeers();
    }
  }

  async function unpair(peer: Peer) {
    if (!window.confirm(`Unpair ${peer.name}? It can't sync with this Device until you pair them again.`)) return;
    await run("unpair", () => syncApi.removePeer(peer.device_id));
    void loadPeers();
  }

  const left = pairing ? expiresIn(pairing.expires_at, now) : null;

  return (
    <section className="card">
      <h2>Sync</h2>
      <p className="muted">
        Sync exchanges changes directly with your other Devices on the same network, encrypted. Nothing goes
        through the internet.
      </p>
      {status && (
        <div className="row">
          <label>
            <input
              type="checkbox"
              checked={status.enabled}
              disabled={busy !== null}
              onChange={(e) => void setServer(e.target.checked)}
            />{" "}
            Allow Sync: let paired Devices connect to this one
          </label>
          <label>
            Port{" "}
            <input
              className="port"
              inputMode="numeric"
              value={port}
              disabled={status.enabled || busy !== null}
              onChange={(e) => setPort(e.target.value)}
            />
          </label>
        </div>
      )}
      {explainFirewall && (
        <div className="notice">
          <p className="small">
            <b>Windows Firewall.</b> Your phone connects to this PC to Sync, and Windows Firewall blocks incoming
            connections unless a rule allows them. The app can add a rule that lets other Devices reach this app on{" "}
            <b>private networks only</b> (like your home Wi-Fi), never on public networks such as a café. It covers
            the Sync port and the network announcement your phone uses to find this PC. Windows will ask for your
            permission.
          </p>
          <div className="actions">
            <button className="primary" disabled={busy !== null} onClick={() => void setServer(true, true)}>
              Add the rule and turn on Sync
            </button>
            <button disabled={busy !== null} onClick={() => void setServer(true, false)}>
              Turn on without it
            </button>
            <button disabled={busy !== null} onClick={() => setExplainFirewall(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {status?.enabled && !status.listening && (
        <p className="error small">Sync is on but the app couldn't listen on port {status.port}. Try another port.</p>
      )}
      {status?.listening && (
        <p className="muted small">
          Listening on {status.addrs.join(", ")} port {status.port}.
          {status.announced && " Announced on the local network, so your phone finds this PC even after its address changes."}
          {status.firewall === "allowed" && " Windows Firewall allows Sync on private networks."}
        </p>
      )}
      {status?.listening && status.firewall === "missing" && (
        <p className="muted small">
          Windows Firewall has no rule for Sync, so your phone may not reach this PC. If Windows asked to allow the
          app, allowing it on private networks also works.{" "}
          <button disabled={busy !== null} onClick={() => void setServer(true, true)}>
            Add firewall rule (private networks only)
          </button>
        </p>
      )}

      <h3>Pair new device</h3>
      {!pairing ? (
        <button disabled={!status?.listening || busy !== null} onClick={() => void startPairing()}>
          Pair new device
        </button>
      ) : left ? (
        <div>
          <p className="small">
            On the phone, choose <b>Pair with PC</b> and scan this QR code. On another PC, paste the code below in
            Settings → Sync. It works once, and expires at{" "}
            {new Date(pairing.expires_at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} (in{" "}
            {left}).
          </p>
          <PairingQr svg={pairing.qr_svg} />
          <textarea readOnly rows={3} className="code" value={pairing.payload} onFocus={(e) => e.target.select()} />
          <div className="actions">
            <button onClick={() => void navigator.clipboard.writeText(pairing.payload)}>Copy</button>
            <button onClick={() => void cancelPairing()}>Cancel</button>
          </div>
        </div>
      ) : (
        <p className="small">
          The pairing code has expired.{" "}
          <button onClick={() => void startPairing()}>Make a new one</button>
        </p>
      )}
      {!status?.listening && !pairing && <p className="muted small">Turn on Allow Sync to pair a new Device.</p>}

      <h3>{mobile ? "Pair with PC" : "Join another Device"}</h3>
      {!mobile && <p className="muted small">Paste the pairing code shown by "Pair new device" on the other Device.</p>}
      <PairWithPc mobile={mobile} onJoined={joined} />

      <h3>Paired Devices</h3>
      {peers.length === 0 ? (
        <p className="muted small">No paired Devices yet.</p>
      ) : (
        <ul className="peers">
          {peers.map((peer) => (
            <li key={peer.device_id}>
              <span className="name">{peer.name}</span>{" "}
              <span className="muted small">
                last Sync {when(peer.last_sync_at)}
                {peer.last_addr ? ` · ${peer.last_addr}` : " · connects to this Device"}
              </span>
              <div className="actions">
                <button
                  disabled={!peer.last_addr || busy !== null}
                  title={peer.last_addr ? undefined : "This Device connects to you: start the Sync on it."}
                  onClick={() => void syncNow(peer)}
                >
                  {busy === `sync:${peer.device_id}` ? "Syncing…" : "Sync now"}
                </button>
                <button disabled={busy !== null} onClick={() => void unpair(peer)}>
                  Unpair
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {progress && <p className="small">{progressText(progress)}</p>}
      {message && <p className="small">{message}</p>}
      {error && <p className="error small">{error}</p>}
      {rescan && (
        <PairWithPc mobile={mobile} onJoined={joined} scanLabel="Rescan the PC's QR code (keeps the Pairing)" />
      )}
    </section>
  );
}
