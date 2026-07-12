import React from "react";
import { changePin } from "../lib/tauri";

export const THEMES = [
  { id: "amphora", label: "Amphora (default)" },
  { id: "purple", label: "Βαθύ μωβ" },
  { id: "ocean", label: "Ωκεανός" },
  { id: "light", label: "Ανοιχτό" },
];

const AUTO_LOCK_OPTIONS = [
  { value: 1, label: "1 λεπτό" },
  { value: 5, label: "5 λεπτά" },
  { value: 15, label: "15 λεπτά" },
  { value: 0, label: "Ποτέ" },
];

export default function Settings({
  theme,
  setTheme,
  autoLockMin,
  setAutoLockMin,
  walletUnlocked,
  onClose,
}: {
  theme: string;
  setTheme: (t: string) => void;
  autoLockMin: number;
  setAutoLockMin: (n: number) => void;
  walletUnlocked: boolean;
  onClose: () => void;
}) {
  const [showPinChange, setShowPinChange] = React.useState(false);
  const [oldPin, setOldPin] = React.useState("");
  const [newPin, setNewPin] = React.useState("");
  const [pinBusy, setPinBusy] = React.useState(false);
  const [pinMsg, setPinMsg] = React.useState("");
  const [pinErr, setPinErr] = React.useState("");

  async function savePin() {
    setPinErr("");
    setPinMsg("");
    if (newPin.length < 6) {
      setPinErr("Το νέο PIN πρέπει να έχει τουλάχιστον 6 χαρακτήρες.");
      return;
    }
    setPinBusy(true);
    try {
      await changePin(oldPin, newPin);
      setPinMsg("Το PIN άλλαξε ✓");
      setOldPin("");
      setNewPin("");
      setTimeout(() => setShowPinChange(false), 1200);
    } catch (e) {
      setPinErr(String(e));
    } finally {
      setPinBusy(false);
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h1>Ρυθμίσεις</h1>

        <h2>Θέμα</h2>
        <div className="theme-grid">
          {THEMES.map((t) => (
            <button
              key={t.id}
              data-theme={t.id}
              className={theme === t.id ? "theme-opt active" : "theme-opt"}
              onClick={() => setTheme(t.id)}
            >
              <span className="theme-swatch">
                <span className="sw-bg" />
                <span className="sw-accent" />
                <span className="sw-text" />
              </span>
              {t.label}
            </button>
          ))}
        </div>

        <h2>Ασφάλεια</h2>
        <div className="security-box">
          <label className="hint">Αυτόματο κλείδωμα μετά από αδράνεια:</label>
          <select value={autoLockMin} onChange={(e) => setAutoLockMin(Number(e.target.value))}>
            {AUTO_LOCK_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>

          {walletUnlocked ? (
            !showPinChange ? (
              <button className="secondary" onClick={() => setShowPinChange(true)}>
                🔐 Αλλαγή PIN
              </button>
            ) : (
              <div className="pin-modal">
                <input
                  type="password"
                  placeholder="Τρέχον PIN"
                  value={oldPin}
                  onChange={(e) => setOldPin(e.target.value)}
                />
                <input
                  type="password"
                  placeholder="Νέο PIN (min 6)"
                  value={newPin}
                  onChange={(e) => setNewPin(e.target.value)}
                />
                <button disabled={pinBusy} onClick={savePin}>
                  {pinBusy ? "..." : "Αποθήκευση"}
                </button>
                <button className="secondary" onClick={() => setShowPinChange(false)}>
                  Άκυρο
                </button>
                {pinMsg && <p className="hint" style={{ color: "var(--ok)" }}>{pinMsg}</p>}
                {pinErr && <p className="error">{pinErr}</p>}
              </div>
            )
          ) : (
            <p className="hint">Ξεκλείδωσε το wallet για να αλλάξεις PIN.</p>
          )}
        </div>

        <button onClick={onClose}>Κλείσιμο</button>
      </div>
    </div>
  );
}
