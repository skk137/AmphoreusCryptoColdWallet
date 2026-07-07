import { useEffect, useState } from "react";
import "./App.css";
import Onboarding from "./routes/Onboarding";
import Dashboard from "./routes/Dashboard";
import { walletStatus } from "./lib/tauri";

function App() {
  const [unlocked, setUnlocked] = useState<boolean | null>(null);

  useEffect(() => {
    walletStatus().then(setUnlocked);
  }, []);

  if (unlocked === null) {
    return (
      <main className="container">
        <p className="hint" style={{ textAlign: "center" }}>
          Φόρτωση...
        </p>
      </main>
    );
  }

  return unlocked ? (
    <Dashboard onLocked={() => setUnlocked(false)} />
  ) : (
    <Onboarding onUnlocked={() => setUnlocked(true)} />
  );
}

export default App;
