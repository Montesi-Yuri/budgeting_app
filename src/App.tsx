import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage } from "./api";
import { viewsToRefresh } from "./sync";
import AccountsPage from "./components/AccountsPage";
import BudgetsPage from "./components/BudgetsPage";
import CategoriesPage from "./components/CategoriesPage";
import CurrencyPicker from "./components/CurrencyPicker";
import HomePage from "./components/HomePage";
import ImportPage from "./components/ImportPage";
import PairWithPc from "./components/PairWithPc";
import PhoneHome from "./components/PhoneHome";
import RestoreBackup from "./components/RestoreBackup";
import SettingsPage from "./components/SettingsPage";
import TransactionsPage from "./components/TransactionsPage";
import UncategorisedPage from "./components/UncategorisedPage";

type View = "home" | "accounts" | "transactions" | "uncategorised" | "categories" | "settings" | "budgets" | "import";

const VIEWS: { view: View; label: string }[] = [
  { view: "home", label: "Home" },
  { view: "accounts", label: "Accounts" },
  { view: "transactions", label: "Transactions" },
  { view: "uncategorised", label: "Uncategorised" },
  { view: "categories", label: "Categories" },
  { view: "settings", label: "Settings" },
  { view: "budgets", label: "Budgets" },
  { view: "import", label: "Import" },
];

/** The phone's tabs: its main uses are quick add and Spendable, on Home. */
const PHONE_VIEWS: View[] = ["home", "transactions", "accounts", "categories", "settings"];

export default function App() {
  // undefined = loading, null = not chosen yet (first launch)
  const [currency, setCurrency] = useState<string | null | undefined>(undefined);
  const [view, setView] = useState<View>("home");
  const [error, setError] = useState<string | null>(null);
  // The phone build gets the phone layout and the QR scanner.
  const [mobile, setMobile] = useState(false);

  // Bumped when a Sync changed what a view shows, so the view refetches.
  const [refreshed, setRefreshed] = useState<Partial<Record<View, number>>>({});

  const loadCurrency = () => api.currencyGet().then(setCurrency, (e) => setError(errorMessage(e)));
  useEffect(() => {
    void loadCurrency();
    api.appInfo().then((info) => setMobile(info.mobile), () => {});
  }, []);

  useEffect(() => {
    const off = listen<{ entities: string[] }>("data://changed", ({ payload }) => {
      if (payload.entities.includes("setting")) api.currencyGet().then(setCurrency, () => {});
      setRefreshed((r) => {
        const next = { ...r };
        for (const v of viewsToRefresh(payload.entities)) next[v] = (next[v] ?? 0) + 1;
        return next;
      });
    });
    return () => void off.then((f) => f());
  }, []);

  async function chooseCurrency(code: string) {
    await api.currencySet(code);
    setCurrency(code);
  }

  if (error) return <main className="page"><p className="error">{error}</p></main>;
  if (currency === undefined) return <main className="page"><p className="muted">Loading…</p></main>;

  if (currency === null) {
    return (
      <main className={mobile ? "page narrow phone" : "page narrow"}>
        <h1>Welcome</h1>
        {mobile && (
          <>
            <p>Pair with your PC to get all your data, including the Currency.</p>
            <PairWithPc mobile onJoined={() => void loadCurrency()} />
            <h2 className="spaced">Or start from scratch</h2>
          </>
        )}
        <p>Choose the Currency for all amounts. There is only one, and it is used everywhere in the app.</p>
        <CurrencyPicker initial="EUR" submitLabel="Continue" onSubmit={chooseCurrency} />
        {!mobile && (
          <>
            <h2 className="spaced">Or pair with another Device</h2>
            <p className="muted small">
              Already using the app on another PC? Pair with it to get all your data, including the Currency.
            </p>
            <PairWithPc mobile={false} onJoined={() => void loadCurrency()} />
          </>
        )}
        <h2 className="spaced">Or restore a Backup</h2>
        <p className="muted small">On a new installation, restore a Backup first to get all your data back.</p>
        <RestoreBackup onRestored={() => void loadCurrency()} />
      </main>
    );
  }

  return (
    <div className={mobile ? "app phone" : "app"}>
      <header className="topbar">
        <span className="brand">Budget</span>
        <nav>
          {VIEWS.filter((v) => !mobile || PHONE_VIEWS.includes(v.view)).map((v) => (
            <button key={v.view} className={view === v.view ? "tab active" : "tab"} onClick={() => setView(v.view)}>
              {v.label}
            </button>
          ))}
        </nav>
      </header>
      <main className="page" key={`${view}-${refreshed[view] ?? 0}`}>
        {view === "home" && (mobile ? <PhoneHome currency={currency} /> : <HomePage currency={currency} />)}
        {view === "accounts" && <AccountsPage currency={currency} />}
        {view === "transactions" && <TransactionsPage currency={currency} />}
        {view === "uncategorised" && <UncategorisedPage currency={currency} />}
        {view === "categories" && <CategoriesPage />}
        {view === "settings" && <SettingsPage currency={currency} onCurrencyChange={chooseCurrency} onRestored={() => void loadCurrency()} />}
        {view === "budgets" && <BudgetsPage currency={currency} />}
        {view === "import" && <ImportPage currency={currency} />}
      </main>
    </div>
  );
}
