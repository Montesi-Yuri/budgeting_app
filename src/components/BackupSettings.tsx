import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, BackupSchedule, errorMessage } from "../api";
import { backupFileName } from "../backup";
import RestoreBackup from "./RestoreBackup";

const FILTERS = [{ name: "Backup", extensions: ["age"] }];

/** "Export backup": enter a passphrase, choose where to save, write one encrypted file. */
function ExportBackup() {
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function exportBackup(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setMessage(null);
    const path = await save({ defaultPath: backupFileName(new Date()), filters: FILTERS });
    if (!path) return;
    setBusy(true);
    try {
      await api.backupExport(path, passphrase);
      setMessage(`Backup saved to ${path}.`);
      setPassphrase("");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="form" onSubmit={exportBackup}>
      <div className="row">
        <label className="field">
          <span>Passphrase</span>
          <input type="password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} />
        </label>
        <button type="submit" className="primary" disabled={busy || !passphrase}>
          {busy ? "Exporting…" : "Export backup…"}
        </button>
      </div>
      {message && <p className="muted">{message}</p>}
      {error && <p className="error">{error}</p>}
    </form>
  );
}

/** The weekly automatic Backup, shown only on the PC. */
function WeeklyBackup() {
  const [schedule, setSchedule] = useState<BackupSchedule | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.backupScheduleGet().then(setSchedule, (e) => setError(errorMessage(e)));
  }, []);

  async function choose() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setFolder(picked);
  }

  async function apply(newFolder: string | null) {
    setBusy(true);
    setError(null);
    try {
      setSchedule(await api.backupScheduleSet(newFolder, newFolder ? passphrase : null));
      setFolder(null);
      setPassphrase("");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  if (error && !schedule) return <p className="error">{error}</p>;
  if (!schedule?.available) return null;
  return (
    <>
      <h3>Weekly Backup</h3>
      <p className="muted small">
        Once a week this PC writes a Backup to a folder you choose (it can be a OneDrive folder) and keeps the last 8.
        The passphrase is kept in the Windows credential store, so it isn't asked again. The folder is a setting of
        this Device only.
      </p>
      {schedule.folder ? (
        <p>
          Folder <code>{schedule.folder}</code>
          {schedule.latest && <span className="muted small"> · latest {schedule.latest}</span>}
        </p>
      ) : (
        <p className="muted">Off.</p>
      )}
      {schedule.last_error && <p className="error">{schedule.last_error}</p>}
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (folder) void apply(folder);
        }}
      >
        <div className="row">
          <button type="button" onClick={choose}>
            Choose folder…
          </button>
          <span className="muted small">{folder ?? ""}</span>
        </div>
        <div className="row">
          <label className="field">
            <span>Passphrase</span>
            <input type="password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} />
          </label>
          <button type="submit" className="primary" disabled={busy || !folder || !passphrase}>
            {schedule.folder ? "Change" : "Turn on"}
          </button>
          {schedule.folder && (
            <button type="button" className="danger" disabled={busy} onClick={() => void apply(null)}>
              Turn off
            </button>
          )}
        </div>
      </form>
      {error && <p className="error">{error}</p>}
    </>
  );
}

/** Settings → Backup: export, restore and the weekly automatic Backup. */
export default function BackupSettings({ onRestored }: { onRestored?: () => void }) {
  return (
    <section className="card">
      <h2>Backup</h2>
      <p className="muted small">
        A Backup is a file with all your data, encrypted with a passphrase. Without the passphrase it can't be
        restored. Restoring adds only the changes this Device doesn't have yet. Paired Devices are not in a Backup:
        after restoring on a new installation, pair them again.
      </p>
      <h3>Export</h3>
      <ExportBackup />
      <h3>Restore</h3>
      <RestoreBackup onRestored={onRestored} />
      <WeeklyBackup />
    </section>
  );
}
