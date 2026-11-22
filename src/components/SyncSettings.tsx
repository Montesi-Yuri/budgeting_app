import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage, PairingInfo, Peer, SyncProgress, SyncReport, SyncStatus } from "../api";
import { expiresIn, progressText, syncSummary } from "../sync";

const when = (unix: number | null) => (unix ? new Date(unix * 1000).toLocaleString() : "never");

/** Sync on/off, Pairing and the paired Devices (Settings page). */
export default function SyncSettings() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [port, setPort] = useState("");
  const [peers, setPeers] = useState<Peer[]>([]);
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [now, setNow] = useState(Date.now());
  const [joinText, setJoinText] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPeers = () => api.peersList().then(setPeers, (e) => setError(errorMessage(e)));

  useEffect(() => {
    api.syncStatus().then((s) => {
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
    try {
      return await action();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  }

  async function setServer(enabled: boolean) {
    const portNumber = Number(port);
    if (!Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65535) {
      setError("Choose a port between 1 and 65535.");
      return;
    }
    const next = await run("server", () => api.syncServerSet(enabled, portNumber));
    if (next) setStatus(next);
    if (!enabled) setPairing(null);
  }

  async function startPairing() {
    const info = await run("pairing", api.pairingStart);
    if (info) {
      setNow(Date.now());
      setPairing(info);
    }
  }

  async function cancelPairing() {
    setPairing(null);
    await api.pairingCancel();
  }

  async function join() {
    const joined = await run("join", () => api.pairingJoin(joinText));
    if (joined) {
      setJoinText("");
      setMessage(`Paired with ${joined.peer.name}. ${syncSummary(joined.report)}`);
      void loadPeers();
    }
  }

  async function syncNow(peer: Peer) {
    const report = await run(`sync:${peer.device_id}`, () => api.syncNow(peer.device_id));
    if (report) {
      setMessage(`${peer.name}: ${syncSummary(report)}`);
      void loadPeers();
    }
  }

  async function unpair(peer: Peer) {
    if (!window.confirm(`Unpair ${peer.name}? It can't sync with this Device until you pair them again.`)) return;
    await run("unpair", () => api.peerRemove(peer.device_id));
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
      {status?.enabled && !status.listening && (
        <p className="error small">Sync is on but the app couldn't listen on port {status.port}. Try another port.</p>
      )}
      {status?.listening && (
        <p className="muted small">
          Listening on {status.addrs.join(", ")} port {status.port}. The first time, Windows may ask to allow the
          app on the network: allow it for private networks.
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
            On the other Device, open Settings → Sync and paste this pairing code. It works once, for {left} more.
          </p>
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

      <h3>Join another Device</h3>
      <p className="muted small">Paste the pairing code shown by "Pair new device" on the other Device.</p>
      <textarea rows={2} className="code" value={joinText} onChange={(e) => setJoinText(e.target.value)} />
      <div className="actions">
        <button disabled={!joinText.trim() || busy !== null} onClick={() => void join()}>
          {busy === "join" ? "Pairing…" : "Pair"}
        </button>
      </div>

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
    </section>
  );
}
