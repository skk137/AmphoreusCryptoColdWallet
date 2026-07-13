import React from "react";
import { changePin } from "../lib/tauri";
import { useT } from "../lib/i18n";

export const THEMES = [
  { id: "amphora", label: "Amphora" },
  { id: "purple", label: "Βαθύ μωβ / Deep purple" },
  { id: "ocean", label: "Ωκεανός / Ocean" },
  { id: "light", label: "Ανοιχτό / Light" },
];

const LANGS = [
  { id: "el", label: "Ελληνικά" },
  { id: "en", label: "English" },
];

export default function Settings({
  theme,
  setTheme,
  autoLockMin,
  setAutoLockMin,
  lang,
  setLang,
  walletUnlocked,
  onClose,
}: {
  theme: string;
  setTheme: (t: string) => void;
  autoLockMin: number;
  setAutoLockMin: (n: number) => void;
  lang: string;
  setLang: (l: string) => void;
  walletUnlocked: boolean;
  onClose: () => void;
}) {
  const { t } = useT();
  const [showPinChange, setShowPinChange] = React.useState(false);
  const [oldPin, setOldPin] = React.useState("");
  const [newPin, setNewPin] = React.useState("");
  const [pinBusy, setPinBusy] = React.useState(false);
  const [pinMsg, setPinMsg] = React.useState("");
  const [pinErr, setPinErr] = React.useState("");

  const autoLockOptions = [
    { value: 1, label: t("al_1") },
    { value: 5, label: t("al_5") },
    { value: 15, label: t("al_15") },
    { value: 0, label: t("al_0") },
  ];

  async function savePin() {
    setPinErr("");
    setPinMsg("");
    if (newPin.length < 6) {
      setPinErr(t("pin_short"));
      return;
    }
    setPinBusy(true);
    try {
      await changePin(oldPin, newPin);
      setPinMsg(t("pin_changed"));
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
        <h1>{t("settings")}</h1>

        {/* LANGUAGE — first setting */}
        <h2>{t("language")}</h2>
        <select value={lang} onChange={(e) => setLang(e.target.value)}>
          {LANGS.map((l) => (
            <option key={l.id} value={l.id}>
              {l.label}
            </option>
          ))}
        </select>

        <h2>{t("theme")}</h2>
        <div className="theme-grid">
          {THEMES.map((th) => (
            <button
              key={th.id}
              data-theme={th.id}
              className={theme === th.id ? "theme-opt active" : "theme-opt"}
              onClick={() => setTheme(th.id)}
            >
              <span className="theme-swatch">
                <span className="sw-bg" />
                <span className="sw-accent" />
                <span className="sw-text" />
              </span>
              {th.label}
            </button>
          ))}
        </div>

        <h2>{t("security")}</h2>
        <div className="security-box">
          <label className="hint">{t("autolock_label")}</label>
          <select value={autoLockMin} onChange={(e) => setAutoLockMin(Number(e.target.value))}>
            {autoLockOptions.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>

          {walletUnlocked ? (
            !showPinChange ? (
              <button className="secondary" onClick={() => setShowPinChange(true)}>
                {t("change_pin")}
              </button>
            ) : (
              <div className="pin-modal">
                <input
                  type="password"
                  placeholder={t("current_pin")}
                  value={oldPin}
                  onChange={(e) => setOldPin(e.target.value)}
                />
                <input
                  type="password"
                  placeholder={t("new_pin_min")}
                  value={newPin}
                  onChange={(e) => setNewPin(e.target.value)}
                />
                <button disabled={pinBusy} onClick={savePin}>
                  {pinBusy ? "..." : t("save")}
                </button>
                <button className="secondary" onClick={() => setShowPinChange(false)}>
                  {t("cancel")}
                </button>
                {pinMsg && <p className="hint" style={{ color: "var(--ok)" }}>{pinMsg}</p>}
                {pinErr && <p className="error">{pinErr}</p>}
              </div>
            )
          ) : (
            <p className="hint">{t("unlock_first")}</p>
          )}
        </div>

        <button onClick={onClose}>{t("close")}</button>
      </div>
    </div>
  );
}
