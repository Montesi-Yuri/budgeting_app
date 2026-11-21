import { useEffect, useState } from "react";
import { api, errorMessage } from "./api";
import AccountsPage from "./components/AccountsPage";
import CategoriesPage from "./components/CategoriesPage";
import CurrencyPicker from "./components/CurrencyPicker";
import SettingsPage from "./components/SettingsPage";
import TransactionsPage from "./components/TransactionsPage";
import UncategorisedPage from "./components/UncategorisedPage";

type View = "accounts" | "transactions" | "uncategorised" | "categories" | "settings";

const VIEWS: { view: View; label: string }[] = [
  { view: "accounts", label: "Accounts" },
  { view: "transactions", label: "Transactions" },
  { view: "uncategorised", label: "Uncategorised" },
  { view: "categories", label: "Categories" },
  { view: "settings", label: "Settings" },
];

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
          {VIEWS.map((v) => (
            <button key={v.view} className={view === v.view ? "tab active" : "tab"} onClick={() => setView(v.view)}>
              {v.label}
            </button>
          ))}
        </nav>
      </header>
      <main className="page">
        {view === "accounts" && <AccountsPage currency={currency} />}
        {view === "transactions" && <TransactionsPage currency={currency} />}
        {view === "uncategorised" && <UncategorisedPage currency={currency} />}
        {view === "categories" && <CategoriesPage />}
        {view === "settings" && <SettingsPage currency={currency} onCurrencyChange={chooseCurrency} />}
      </main>
    </div>
  );
}
