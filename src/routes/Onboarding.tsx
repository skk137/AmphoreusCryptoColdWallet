import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Logo from "../components/Logo";
import {
  DriveInfo,
  createWallet,
  importWallet,
  listRemovableDrives,
  localFolderInfo,
  unlockWallet,
} from "../lib/tauri";

function pinStrength(pin: string): { score: number; label: string; color: string } {
    if (!pin) return { score: 0, label: "", color: "transparent" };
    let score = 0;
    if (pin.length >= 6) score++;
    if (pin.length >= 10) score++;
    if (pin.length >= 16) score++;
    if (/[a-zα-ω]/.test(pin) && /[A-ZΑ-Ω]/.test(pin)) score++;
    if (/[0-9]/.test(pin)) score++;
    if (/[^a-zA-Z0-9α-ωΑ-Ω]/.test(pin)) score++;
    // Κοινά/προβλέψιμα PIN μηδενίζουν
    if (/^(123456|12345678|password|qwerty|111111|000000)/i.test(pin)) score = 0;

    if (score <= 2) return { score, label: "Αδύναμο", color: "#c0392b" };
    if (score <= 4) return { score, label: "Μέτριο", color: "#e67e22" };
    return { score, label: "Δυνατό", color: "#27ae60" };
}

function PinStrengthBar({ pin }: { pin: string }) {
    const s = pinStrength(pin);
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
      setError("Κάποια λέξη δεν ταιριάζει. Έλεγξε το backup σου.");
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
      const selected = await open({ directory: true, title: "Επιλογή φακέλου για το wallet" });
      if (typeof selected !== "string") return;
      pickDrive(await localFolderInfo(selected));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCreate() {
    if (!drive) return;
    if (pin.length < 6) {
      setError("Το PIN πρέπει να έχει τουλάχιστον 6 χαρακτήρες.");
      return;
    }
    if (pin !== pinConfirm) {
      setError("Τα PIN δεν ταιριάζουν.");
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
      setError("Το PIN πρέπει να έχει τουλάχιστον 6 χαρακτήρες.");
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
          Επίλεξε το USB stick που θα χρησιμοποιηθεί για το wallet.
        </p>
        <button onClick={refreshDrives}>Ανανέωση λίστας</button>
        <ul className="drive-list">
          {drives.map((d) => (
            <li key={d.mount_point}>
              <button onClick={() => pickDrive(d)}>
                {d.name || d.mount_point} ({d.mount_point}) —{" "}
                {d.has_wallet ? "υπάρχει wallet" : "άδειο"}
              </button>
            </li>
          ))}
          {drives.length === 0 && <li>Δεν βρέθηκε αφαιρούμενο USB drive.</li>}
        </ul>
        <button className="secondary" onClick={pickLocalFolder}>
          Επιλογή τοπικού φακέλου (δεν προτείνεται)
        </button>
        <p className="hint">
          Η επιλογή τοπικού φακέλου, αποδομεί τη λογική του Cold Wallet — καθώς, το seed μένει στον ίδιο δίσκο με το λειτουργικό, και
            συνεπώς εφόσον η συσκευή ειναι συνδεδεμένη στο διαδίκτυο, το wallet παραμένει hot και ευάλωτο σε Κυβερνοεπιθέσεις.
        </p>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

  if (mode === "choose-action") {
    return (
      <main className="container">
        <h1>{drive?.mount_point}</h1>
        <p>Δεν βρέθηκε wallet σε αυτό το drive.</p>
        <button onClick={() => setMode("create-pin")}>Δημιουργία νέου wallet</button>
        <button onClick={() => setMode("import")}>Επαναφορά από υπάρχον mnemonic</button>
        <button onClick={() => setMode("pick-drive")}>Πίσω</button>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

    if (mode === "create-pin") {
        return (
            <main className="container">
                <h1>Ορισμός PIN</h1>
                <p>Το PIN κρυπτογραφεί το seed που θα αποθηκευτεί στο USB.</p>
                <input type="password" placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
                <PinStrengthBar pin={pin} />
                <input
                    type="password"
                    placeholder="Επιβεβαίωση PIN"
                    value={pinConfirm}
                    onChange={(e) => setPinConfirm(e.target.value)}
                />
                <button disabled={busy} onClick={handleCreate}>
                    {busy ? "..." : "Δημιουργία wallet"}
                </button>
                {error && <p className="error">{error}</p>}
            </main>
        );
    }

  if (mode === "backup") {
    return (
      <main className="container">
        <h1>Backup Phrase</h1>
        <p className="warning">
          Γράψε αυτή τη φράση σε χαρτί και φύλαξέ τη αλλού εκτός από το USB. Δεν θα ξαναφανεί.
        </p>
        <p className="mnemonic">{phrase}</p>
        <button onClick={startVerify}>Την έγραψα, συνέχεια</button>
      </main>
    );
  }

  if (mode === "verify-backup") {
    const words = phrase.trim().split(/\s+/);
    return (
      <main className="container">
        <h1>Επιβεβαίωση backup</h1>
        <p className="hint">
          Για να σιγουρευτούμε ότι έγραψες σωστά τη φράση, πληκτρολόγησε τις λέξεις που ζητάει.
        </p>
        {verifyIdx.map((i) => (
          <input
            key={i}
            placeholder={`Λέξη #${i + 1}`}
            value={verifyInputs[i] ?? ""}
            onChange={(e) => setVerifyInputs({ ...verifyInputs, [i]: e.target.value })}
          />
        ))}
        <button onClick={checkVerify}>Επιβεβαίωση</button>
        <button className="secondary" onClick={() => setMode("backup")}>
          Πίσω να τη δω ξανά
        </button>
        <button
          className="secondary"
          onClick={() => {
            setPhrase("");
            onUnlocked();
          }}
        >
          Skip (μόνο για δοκιμή — μη το κάνεις με πραγματικά χρήματα)
        </button>
        {error && <p className="error">{error}</p>}
        {/* words referenced above via checkVerify closure */}
        {words.length === 0 && <p className="error">Λείπει η φράση.</p>}
      </main>
    );
  }

  if (mode === "import") {
    return (
      <main className="container">
        <h1>Επαναφορά wallet</h1>
        <textarea
          placeholder="24-word mnemonic"
          value={importPhrase}
          onChange={(e) => setImportPhrase(e.target.value)}
        />
        <input type="password" placeholder="Νέο PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
        <button disabled={busy} onClick={handleImport}>
          {busy ? "..." : "Επαναφορά"}
        </button>
        <button onClick={() => setMode("choose-action")}>Πίσω</button>
        {error && <p className="error">{error}</p>}
      </main>
    );
  }

  // mode === "unlock-pin"
  return (
    <main className="container">
      <h1>Ξεκλείδωμα wallet</h1>
      <p>{drive?.mount_point}</p>
      <input type="password" placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
      <button disabled={busy} onClick={handleUnlock}>
        {busy ? "..." : "Ξεκλείδωμα"}
      </button>
      <button onClick={() => setMode("pick-drive")}>Πίσω</button>
      {error && <p className="error">{error}</p>}
    </main>
  );
}
