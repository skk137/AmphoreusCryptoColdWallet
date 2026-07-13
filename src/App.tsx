import { useEffect, useState } from "react";
import "./App.css";
import Onboarding from "./routes/Onboarding";
import Dashboard from "./routes/Dashboard";
import Settings from "./components/Settings";
import { walletStatus } from "./lib/tauri";
import { Lang, LangContext, makeT } from "./lib/i18n";

function App() {
  const [unlocked, setUnlocked] = useState<boolean | null>(null);
  const [theme, setTheme] = useState(() => localStorage.getItem("theme") || "amphora");
  const [autoLockMin, setAutoLockMin] = useState(() =>
    Number(localStorage.getItem("autoLockMin") ?? "15")
  );
  const [lang, setLang] = useState(() => localStorage.getItem("lang") || "el");
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    walletStatus().then(setUnlocked);
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem("autoLockMin", String(autoLockMin));
  }, [autoLockMin]);

  useEffect(() => {
    localStorage.setItem("lang", lang);
  }, [lang]);

  const t = makeT(lang as Lang);

  let content;
  if (unlocked === null) {
    content = (
      <main className="container">
        <p className="hint" style={{ textAlign: "center" }}>
          {t("loading")}
        </p>
      </main>
    );
  } else if (unlocked) {
    content = <Dashboard onLocked={() => setUnlocked(false)} autoLockMin={autoLockMin} />;
  } else {
    content = <Onboarding onUnlocked={() => setUnlocked(true)} />;
  }

  return (
    <LangContext.Provider value={{ lang: lang as Lang, t }}>
      <button className="gear" title={t("settings")} onClick={() => setSettingsOpen(true)}>
        ⚙
      </button>
      {content}
      {settingsOpen && (
        <Settings
          theme={theme}
          setTheme={setTheme}
          autoLockMin={autoLockMin}
          setAutoLockMin={setAutoLockMin}
          lang={lang}
          setLang={setLang}
          walletUnlocked={unlocked === true}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </LangContext.Provider>
  );
}

export default App;
