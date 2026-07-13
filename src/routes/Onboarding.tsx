import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Logo from "../components/Logo";
import { TFunc, useT } from "../lib/i18n";
import {
  DriveInfo,
  createWallet,
  importWallet,
  listRemovableDrives,
  localFolderInfo,
  unlockWallet,
} from "../lib/tauri";

function pinStrength(pin: string, t: TFunc): { score: number; label: string; color: string } {
    if (!pin) return { score: 0, label: "", color: "transparent" };
    let score = 0;
    if (pin.length >= 6) score++;
    if (pin.length >= 10) score++;
    if (pin.length >= 16) score++;
    if (/[a-zα-ω]/.test(pin) && /[A-ZΑ-Ω]/.test(pin)) score++;
    if (/[0-9]/.test(pin)) score++;
    if (/[^a-zA-Z0-9α-ωΑ-Ω]/.test(pin)) score++;
    // Common/predictable PINs reset to zero
    if (/^(123456|12345678|password|qwerty|111111|000000)/i.test(pin)) score = 0;

    if (score <= 2) return { score, label: t("pin_weak"), color: "#c0392b" };
    if (score <= 4) return { score, label: t("pin_medium"), color: "#e67e22" };
    return { score, label: t("pin_strong"), color: "#27ae60" };
}

function PinStrengthBar({ pin }: { pin: string }) {
    const { t } = useT();
    const s = pinStrength(pin, t);
    if (!pin) return null;
    return (
        <div className="strength">
            <div className="strength-track">
                <div
                    className="strength-fill"
                    style={{ width: `${Math.min(100, (s.score / 6) * 100)}%`, background: s.color }}
                />
            </div>
            <span style={{ color: s.color }}>{s.label}</span>
        </div>
    );
}

type Mode =
  | "pick-drive"
  | "choose-action"
  | "create-pin"
  | "backup"
  | "verify-backup"
  | "import"
  | "unlock-pin";

export default function Onboarding({ onUnlocked }: { onUnlocked: () => void }) {
  const { t } = useT();
  const [mode, setMode] = useState<Mode>("pick-drive");
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [drive, setDrive] = useState<DriveInfo | null>(null);
  const [pin, setPin] = useState("");
  const [pinConfirm, setPinConfirm] = useState("");
  const [phrase, setPhrase] = useState("");
  const [importPhrase, setImportPhrase] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // Seed-backup verification: 3 random word positions the user must retype.
  const [verifyIdx, setVerifyIdx] = useState<number[]>([]);
  const [verifyInputs, setVerifyInputs] = useState<Record<number, string>>({});

  function startVerify() {
    const words = phrase.trim().split(/\s+/);
    // pick 3 distinct random indices
    const idx = new Set<number>();
    while (idx.size < 3 && idx.size < words.length) {
      idx.add(Math.floor(Math.random() * words.length));
    }
    setVerifyIdx([...idx].sort((a, b) => a - b));
    setVerifyInputs({});
    setError("");
    setMode("verify-backup");
  }

  function checkVerify() {
    const words = phrase.trim().split(/\s+/);
    const allOk = verifyIdx.every(
      (i) => (verifyInputs[i] ?? "").trim().toLowerCase() === words[i]
    );
    if (!allOk) {
      setError(t("word_mismatch"));
      return;
    }
    setPhrase("");
    onUnlocked();
  }

  async function refreshDrives() {
    setError("");
    try {
      setDrives(await listRemovableDrives());
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    refreshDrives();
  }, []);

  function pickDrive(d: DriveInfo) {
    setDrive(d);
    setError("");
    setMode(d.has_wallet ? "unlock-pin" : "choose-action");
  }

  async function pickLocalFolder() {
    setError("");
    try {
      const selected = await open({ directory: true, title: t("pick_folder_title") });
      if (typeof selected !== "string") return;
      pickDrive(await localFolderInfo(selected));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCreate() {
    if (!drive) return;
    if (pin.length < 6) {
      setError(t("pin_min6"));
      return;
    }
    if (pin !== pinConfirm) {
      setError(t("pin_mismatch"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      const mnemonic = await createWallet(drive.mount_point, pin);
      setPhrase(mnemonic);
      setMode("backup");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleImport() {
    if (!drive) return;
    if (pin.length < 6) {
      setError(t("pin_min6"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      await importWallet(drive.mount_point, pin, importPhrase);
      await unlockWallet(drive.mount_point, pin);
      onUnlocked();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleUnlock() {
    if (!drive) return;
    setBusy(true);
    setError("");
    try {
      await unlockWallet(drive.mount_point, pin);
      onUnlocked();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
    // ΑΡΧΙΚΗ ΟΘΟΝΗ STARTING SCREEN
  if (mode === "pick-drive") {
    return (
      <main className="container">
        <Logo />
        <p className="hint" style={{ textAlign: "center" }}>
          {t("pick_usb")}
        </p>
        <button onClick={refreshDrives}>{t("refresh_list")}</button>
        <ul className="drive-list">
          {drives.map((d) => (
            <li key={d.mount_point}>
              <button onClick={() => pickDrive(d)}>
                {d.name || d.mount_point} ({d.mount_point}) —{" "}
                {d.has_wallet ? t("has_wallet") : t("empty")}
              </button>
            </li>
          ))}
          {drives.length === 0 && <li>{t("no_usb")}</li>}
        </ul>
        <button className="secondary" onClick={pickLocalFolder}>
          {t("pick_local")}
        </button>
        <p className="hint">{t("local_warning")}</p>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

  if (mode === "choose-action") {
    return (
      <main className="container">
        <h1>{drive?.mount_point}</h1>
        <p>{t("no_wallet_on_drive")}</p>
        <button onClick={() => setMode("create-pin")}>{t("create_new")}</button>
        <button onClick={() => setMode("import")}>{t("restore_existing")}</button>
        <button onClick={() => setMode("pick-drive")}>{t("back")}</button>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

    if (mode === "create-pin") {
        return (
            <main className="container">
                <h1>{t("set_pin")}</h1>
                <p>{t("pin_encrypts")}</p>
                <input type="password" placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
                <PinStrengthBar pin={pin} />
                <input
                    type="password"
                    placeholder={t("confirm_pin")}
                    value={pinConfirm}
                    onChange={(e) => setPinConfirm(e.target.value)}
                />
                <button disabled={busy} onClick={handleCreate}>
                    {busy ? "..." : t("create_wallet_btn")}
                </button>
                {error && <p className="error">{error}</p>}
            </main>
        );
    }

  if (mode === "backup") {
    return (
      <main className="container">
        <h1>{t("backup_phrase")}</h1>
        <p className="warning">{t("backup_warning")}</p>
        <p className="mnemonic">{phrase}</p>
        <button onClick={startVerify}>{t("wrote_it")}</button>
      </main>
    );
  }

  if (mode === "verify-backup") {
    const words = phrase.trim().split(/\s+/);
    return (
      <main className="container">
        <h1>{t("verify_backup_title")}</h1>
        <p className="hint">{t("verify_hint")}</p>
        {verifyIdx.map((i) => (
          <input
            key={i}
            placeholder={t("word_n", i + 1)}
            value={verifyInputs[i] ?? ""}
            onChange={(e) => setVerifyInputs({ ...verifyInputs, [i]: e.target.value })}
          />
        ))}
        <button onClick={checkVerify}>{t("verify")}</button>
        <button className="secondary" onClick={() => setMode("backup")}>
          {t("back_to_see")}
        </button>
        <button
          className="secondary"
          onClick={() => {
            setPhrase("");
            onUnlocked();
          }}
        >
          {t("skip_test")}
        </button>
        {error && <p className="error">{error}</p>}
        {words.length === 0 && <p className="error">{t("missing_phrase")}</p>}
      </main>
    );
  }

  if (mode === "import") {
    return (
      <main className="container">
        <h1>{t("restore_wallet")}</h1>
        <textarea
          placeholder={t("mnemonic_ph")}
          value={importPhrase}
          onChange={(e) => setImportPhrase(e.target.value)}
        />
        <input type="password" placeholder={t("new_pin")} value={pin} onChange={(e) => setPin(e.target.value)} />
        <button disabled={busy} onClick={handleImport}>
          {busy ? "..." : t("restore_btn")}
        </button>
        <button onClick={() => setMode("choose-action")}>{t("back")}</button>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

  // mode === "unlock-pin"
  return (
    <main className="container">
      <h1>{t("unlock_wallet")}</h1>
      <p>{drive?.mount_point}</p>
      <input type="password" placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
      <button disabled={busy} onClick={handleUnlock}>
        {busy ? "..." : t("unlock_btn")}
      </button>
      <button onClick={() => setMode("pick-drive")}>{t("back")}</button>
      {error && <p className="error">{error}</p>}
    </main>
  );
}
