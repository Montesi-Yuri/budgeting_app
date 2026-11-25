import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { errorMessage, backupApi } from "@/shared/api";
import { restoreSummary } from "../lib/backup";

const FILTERS = [{ name: "Backup", extensions: ["age"] }];

type Props = { onRestored?: () => void };

/** "Restore backup": choose the file, enter its passphrase, apply every change in it. */
export default function RestoreBackup({ onRestored }: Props) {
  const [path, setPath] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function choose() {
    const picked = await open({ multiple: false, directory: false, filters: FILTERS });
    if (typeof picked === "string") setPath(picked);
  }

  async function restore(e: React.FormEvent) {
    e.preventDefault();
    if (!path) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const report = await backupApi.restore(path, passphrase);
      setMessage(restoreSummary(report));
      setPassphrase("");
      onRestored?.();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="form" onSubmit={restore}>
      <div className="row">
        <button type="button" onClick={choose}>
          Choose Backup file…
        </button>
        <span className="muted small">{path ?? "No file chosen"}</span>
      </div>
      <div className="row">
        <label className="field">
          <span>Passphrase</span>
          <input type="password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} />
        </label>
        <button type="submit" className="primary" disabled={busy || !path || !passphrase}>
          {busy ? "Restoring…" : "Restore backup"}
        </button>
      </div>
      {message && <p className="muted">{message}</p>}
      {error && <p className="error">{error}</p>}
    </form>
  );
}
