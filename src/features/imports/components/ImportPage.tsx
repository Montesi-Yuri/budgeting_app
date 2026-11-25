import { useCallback, useEffect, useState } from "react";
import { AccountWithBalance, errorMessage, ImportBatch, ImportPreview, ImportProfile, ImportSummary, accountsApi, importsApi } from "@/shared/api";
import { formatMinor } from "@/shared/lib/money";
import { Money } from "@/shared/ui";
import { guessColumns, guessDelimiter, importResultText } from "../lib/imports";
import ImportProfileForm from "./ImportProfileForm";

type ChosenFile = { name: string; bytes: Uint8Array };

/**
 * Import a bank CSV into an Account. The first time, the user sets up the
 * Account's Import profile with a live preview; later Imports reuse it.
 * The file is only read here and sent to the core; it is never stored.
 */
export default function ImportPage({ currency }: { currency: string }) {
  const [accounts, setAccounts] = useState<AccountWithBalance[] | null>(null);
  const [accountId, setAccountId] = useState<string | null>(null);
  // undefined = loading
  const [profile, setProfile] = useState<ImportProfile | null | undefined>(undefined);
  const [history, setHistory] = useState<ImportBatch[]>([]);
  const [file, setFile] = useState<ChosenFile | null>(null);
  /** The profile being set up or edited; null = not in the setup step. */
  const [draft, setDraft] = useState<ImportProfile | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    accountsApi.list().then((accs) => {
      setAccounts(accs);
      setAccountId((current) => current ?? accs[0]?.id ?? null);
    }, (e) => setError(errorMessage(e)));
  }, []);

  const loadAccount = useCallback(() => {
    if (!accountId) return;
    setProfile(undefined);
    importsApi.profileGet(accountId).then(setProfile, (e) => setError(errorMessage(e)));
    importsApi.history(accountId).then(setHistory, (e) => setError(errorMessage(e)));
  }, [accountId]);
  useEffect(loadAccount, [loadAccount]);

  // The live preview: re-read the file whenever the profile in use changes.
  const inUse = draft ?? profile ?? null;
  useEffect(() => {
    if (!file || !inUse) {
      setPreview(null);
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      importsApi.preview(inUse, file.bytes).then((p) => current && setPreview(p), (e) => current && setError(errorMessage(e)));
    }, 150);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [file, inUse]);

  function changeAccount(id: string) {
    setAccountId(id);
    setFile(null);
    setDraft(null);
    setSummary(null);
    setError(null);
  }

  async function chooseFile(input: HTMLInputElement) {
    const chosen = input.files?.[0];
    input.value = "";
    if (!chosen || !accountId) return;
    setSummary(null);
    setError(null);
    const bytes = new Uint8Array(await chosen.arrayBuffer());
    setFile({ name: chosen.name, bytes });
    if (profile === null) {
      // No Import profile yet: start the setup step with first guesses.
      try {
        let start = await importsApi.profileDraft(accountId);
        const first = await importsApi.preview(start, bytes);
        const header = first.first_lines[start.skip_rows];
        if (header !== undefined) start = { ...start, delimiter: guessDelimiter(header) };
        const second = await importsApi.preview(start, bytes);
        setDraft(guessColumns(start, second.columns));
      } catch (e) {
        setError(errorMessage(e));
      }
    }
  }

  async function saveProfile() {
    if (!draft) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await importsApi.profileSave(draft);
      setProfile(saved);
      setDraft(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function runImport() {
    if (!file || !accountId) return;
    setBusy(true);
    setError(null);
    try {
      setSummary(await importsApi.run(accountId, file.name, file.bytes));
      setFile(null);
      importsApi.history(accountId).then(setHistory, (e) => setError(errorMessage(e)));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  if (accounts && accounts.length === 0) {
    return (
      <>
        <h1>Import</h1>
        <p className="muted">Add an Account first.</p>
      </>
    );
  }

  const canImport = !!file && !!profile && !draft && !!preview && !preview.problem && preview.rows_with_errors === 0 && preview.total_rows > 0;

  return (
    <>
      <div className="heading">
        <h1>Import</h1>
        {profile && !draft && (
          <button onClick={() => setDraft(profile)}>Edit Import profile</button>
        )}
      </div>
      {error && <p className="error">{error}</p>}
      {accounts && (
        <div className="row toolbar">
          <label className="field">
            <span>Account</span>
            <select value={accountId ?? ""} onChange={(e) => changeAccount(e.target.value)}>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Bank CSV file</span>
            <input type="file" accept=".csv,.txt,text/csv" onChange={(e) => chooseFile(e.currentTarget)} />
          </label>
        </div>
      )}
      {profile === null && !file && (
        <p className="muted">This Account has no Import profile yet. Choose a CSV file to set one up.</p>
      )}
      {profile && !draft && <p className="muted small">Using the Import profile “{profile.name}”.</p>}

      {summary && <ImportResult summary={summary} currency={currency} />}

      {draft && (
        <ImportProfileForm
          profile={draft}
          preview={preview}
          onChange={setDraft}
          onSave={saveProfile}
          onCancel={profile ? () => setDraft(null) : null}
          busy={busy}
        />
      )}

      {file && preview && <PreviewTable preview={preview} currency={currency} fileName={file.name} />}

      {canImport && (
        <div className="actions">
          <button className="primary" disabled={busy} onClick={runImport}>
            Import {preview!.total_rows} lines into this Account
          </button>
        </div>
      )}

      <h2>Import history</h2>
      {history.length === 0 && <p className="muted">No Imports into this Account yet.</p>}
      {history.length > 0 && (
        <ul className="list">
          {history.map((b) => (
            <li key={b.id} className="row-item">
              <span className="date muted small">{new Date(b.imported_at).toLocaleString()}</span>
              <span className="grow">
                <span className="name">{b.source_name}</span>
                <span className="muted small">
                  {b.first_date && ` · ${b.first_date} to ${b.last_date}`}
                </span>
              </span>
              <span className="small">
                {b.rows_new} new · {b.rows_skipped} skipped
              </span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

function ImportResult({ summary, currency }: { summary: ImportSummary; currency: string }) {
  const m = summary.balance_mismatch;
  return (
    <div className="card">
      <h2>Imported {summary.batch.source_name}</h2>
      <p>{importResultText(summary.batch.rows_new, summary.batch.rows_skipped)}</p>
      {m && (
        <p className="error">
          The balance doesn't match from line {m.line} ({m.date}, {m.description}): the file says{" "}
          {formatMinor(m.file_balance_minor, currency)}, the app calculates {formatMinor(m.calculated_balance_minor, currency)}.
          Check the Opening balance and for Transactions missing or entered twice.
        </p>
      )}
    </div>
  );
}

function PreviewTable({ preview, currency, fileName }: { preview: ImportPreview; currency: string; fileName: string }) {
  if (preview.problem) {
    return (
      <div className="card">
        <h2>Preview of {fileName}</h2>
        <p className="error">{preview.problem}</p>
        <p className="muted small">The first lines of the file:</p>
        <pre className="small">{preview.first_lines.map((l, i) => `${i + 1}: ${l}`).join("\n")}</pre>
      </div>
    );
  }
  return (
    <div className="card">
      <h2>Preview of {fileName}</h2>
      <p className="muted small">
        The first {preview.rows.length} of {preview.total_rows} lines, as they would be imported.
        {preview.rows_with_errors > 0 && ` ${preview.rows_with_errors} lines can't be read; fix the profile before importing.`}
      </p>
      <ul className="list">
        {preview.rows.map((r) => (
          <li key={r.line} className="row-item">
            <span className="date muted small">
              {r.line}: {r.date ?? "?"}
            </span>
            <span className="grow">
              <span className="name">{r.description}</span>
              {r.balance_after_minor !== null && (
                <span className="muted small"> · balance {formatMinor(r.balance_after_minor, currency)}</span>
              )}
              {r.error && <span className="error small"> {r.error}</span>}
            </span>
            {r.amount_minor !== null ? <Money minor={r.amount_minor} currency={currency} /> : <span className="muted">?</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}
