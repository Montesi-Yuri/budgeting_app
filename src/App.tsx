import { useEffect, useState } from "react";
import { api, errorMessage } from "./api";
import AccountsPage from "./components/AccountsPage";
import CurrencyPicker from "./components/CurrencyPicker";
import SettingsPage from "./components/SettingsPage";

type View = "accounts" | "settings";

export default function App() {
  // undefined = loading, null = not chosen yet (first launch)
  const [currency, setCurrency] = useState<string | null | undefined>(undefined);
  const [view, setView] = useState<View>("accounts");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.currencyGet().then(setCurrency, (e) => setError(errorMessage(e)));
  }, []);

  async function chooseCurrency(code: string) {
    await api.currencySet(code);
    setCurrency(code);
  }

  if (error) return <main className="page"><p className="error">{error}</p></main>;
  if (currency === undefined) return <main className="page"><p className="muted">Loading…</p></main>;

  if (currency === null) {
    return (
      <main className="page narrow">
        <h1>Welcome</h1>
        <p>Choose the Currency for all amounts. There is only one, and it is used everywhere in the app.</p>
        <CurrencyPicker initial="EUR" submitLabel="Continue" onSubmit={chooseCurrency} />
      </main>
    );
  }

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Budget</span>
        <nav>
          <button className={view === "accounts" ? "tab active" : "tab"} onClick={() => setView("accounts")}>
            Accounts
          </button>
          <button className={view === "settings" ? "tab active" : "tab"} onClick={() => setView("settings")}>
            Settings
          </button>
        </nav>
      </header>
      <main className="page">
        {view === "accounts" ? (
          <AccountsPage currency={currency} />
        ) : (
          <SettingsPage currency={currency} onCurrencyChange={chooseCurrency} />
        )}
      </main>
    </div>
  );
}
