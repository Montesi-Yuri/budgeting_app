import { RestoreBackup } from "@/features/backup";
import { CurrencyPicker } from "@/features/settings";
import { PairWithPc } from "@/features/sync";

type Props = {
  /** The phone build: pairing with the PC comes first. */
  mobile: boolean;
  /** Pairing or a restore brought the Currency (and all data) over. */
  onReady: () => void;
  onCurrencyChosen: (code: string) => Promise<void>;
};

/** First launch: no Currency yet. Choose one, pair with another Device, or restore a Backup. */
export default function Welcome({ mobile, onReady, onCurrencyChosen }: Props) {
  return (
    <main className={mobile ? "page narrow phone" : "page narrow"}>
      <h1>Welcome</h1>
      {mobile && (
        <>
          <p>Pair with your PC to get all your data, including the Currency.</p>
          <PairWithPc mobile onJoined={onReady} />
          <h2 className="spaced">Or start from scratch</h2>
        </>
      )}
      <p>Choose the Currency for all amounts. There is only one, and it is used everywhere in the app.</p>
      <CurrencyPicker initial="EUR" submitLabel="Continue" onSubmit={onCurrencyChosen} />
      {!mobile && (
        <>
          <h2 className="spaced">Or pair with another Device</h2>
          <p className="muted small">
            Already using the app on another PC? Pair with it to get all your data, including the Currency.
          </p>
          <PairWithPc mobile={false} onJoined={onReady} />
        </>
      )}
      <h2 className="spaced">Or restore a Backup</h2>
      <p className="muted small">On a new installation, restore a Backup first to get all your data back.</p>
      <RestoreBackup onRestored={onReady} />
    </main>
  );
}
