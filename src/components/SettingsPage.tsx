import { useEffect, useState } from "react";
import { api, AppInfo } from "../api";
import BackupSettings from "./BackupSettings";
import CurrencyPicker from "./CurrencyPicker";
import SavingsTargetForm from "./SavingsTargetForm";

type Props = {
  currency: string;
  onCurrencyChange: (code: string) => Promise<void>;
  /** After a Backup is restored, data shown elsewhere may have changed. */
  onRestored?: () => void;
};

export default function SettingsPage({ currency, onCurrencyChange, onRestored }: Props) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    api.appInfo().then(setInfo, () => setInfo(null));
  }, []);

  return (
    <>
      <h1>Settings</h1>
      <section className="card">
        <h2>Currency</h2>
        <p className="muted">
          Every amount is shown in this Currency. Changing it does not convert amounts that are already entered.
        </p>
        <CurrencyPicker
          key={currency}
          initial={currency}
          submitLabel="Save"
          onSubmit={async (code) => {
            await onCurrencyChange(code);
            setSaved(true);
          }}
        />
        {saved && <p className="muted">Saved.</p>}
      </section>
      <SavingsTargetForm currency={currency} />
      <BackupSettings onRestored={onRestored} />
      {info && (
        <section className="card">
          <h2>This Device</h2>
          <p className="muted small">
            Device id <code>{info.device_id}</code> · schema {info.schema_version}
          </p>
        </section>
      )}
    </>
  );
}
