import { useEffect, useState } from "react";
import Logo from "../components/Logo";
import {
  Addresses,
  Balances,
  getAddresses,
  getBalances,
  lockWallet,
  sendBtc,
  sendSol,
  sendUsdc,
} from "../lib/tauri";

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className="copy"
      onClick={async () => {
        await navigator.clipboard.writeText(value);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
    >
      {copied ? "Αντιγράφηκε ✓" : "Αντιγραφή"}
    </button>
  );
}

function SendForm({
  label,
  unit,
  decimals,
  explorerBase,
  onSend,
  onSent,
}: {
  label: string;
  unit: string;
  decimals: number;
  explorerBase: string;
  onSend: (to: string, baseUnits: number) => Promise<string>;
  onSent: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const [sending, setSending] = useState(false);
  const [result, setResult] = useState("");
  const [error, setError] = useState("");

  async function handleSend() {
    setError("");
    setResult("");
    const parsed = parseFloat(amount.replace(",", "."));
    if (!to.trim()) {
      setError("Συμπλήρωσε τη διεύθυνση παραλήπτη.");
      return;
    }
    if (!parsed || parsed <= 0) {
      setError("Μη έγκυρο ποσό.");
      return;
    }
    const baseUnits = Math.round(parsed * Math.pow(10, decimals));
    const ok = window.confirm(
      `Επιβεβαίωση αποστολής:\n\n${parsed} ${unit}\n\nπρος: ${to.trim()}\n\nΗ συναλλαγή δεν μπορεί να ανακληθεί. Συνέχεια;`
    );
    if (!ok) return;
    setSending(true);
    try {
      const txid = await onSend(to.trim(), baseUnits);
      setResult(txid);
      setTo("");
      setAmount("");
      onSent();
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  }

  if (!open) {
    return (
      <button className="copy" onClick={() => setOpen(true)}>
        Αποστολή {label}
      </button>
    );
  }

  return (
    <div className="send-form">
      <input
        placeholder="Διεύθυνση παραλήπτη"
        value={to}
        onChange={(e) => setTo(e.target.value)}
      />
      <input
        placeholder={`Ποσό σε ${unit}`}
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
      />
      <button disabled={sending} onClick={handleSend}>
        {sending ? "Αποστολή..." : `Αποστολή ${unit}`}
      </button>
      <button className="copy" onClick={() => setOpen(false)}>
        Άκυρο
      </button>
      {result && (
        <p className="hint">
          Εστάλη! ID συναλλαγής: <span className="address">{result}</span>
          <br />
          <a href={`${explorerBase}${result}`} target="_blank" rel="noreferrer">
            Προβολή συναλλαγής στο Chrome!
          </a>
        </p>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

export default function Dashboard({ onLocked }: { onLocked: () => void }) {
  const [addresses, setAddresses] = useState<Addresses | null>(null);
  const [balances, setBalances] = useState<Balances | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);

  async function load() {
    setError("");
    setLoading(true);
    try {
      const addrs = await getAddresses();
      setAddresses(addrs);
      setBalances(await getBalances(addrs.btc, addrs.sol, addrs.evm));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, []);

  async function handleLock() {
    setBusy(true);
    try {
      await lockWallet();
    } finally {
      setBusy(false);
      onLocked();
    }
  }

  return (
    <main className="container">
      <Logo size={32} />
      {addresses && <p className="badge">{addresses.network} — δοκιμαστικά δίκτυα </p>}

      <div className="card">
        <h2>Bitcoin (testnet)</h2>
        {addresses ? (
          <>
            <p className="address">{addresses.btc}</p>
            <CopyButton value={addresses.btc} />
            <p className="balance">
              {balances ? `${(balances.btc_sats / 1e8).toFixed(8)} tBTC` : loading ? "..." : "—"}
            </p>
            {balances && balances.btc_pending_sats !== 0 && (
              <p className="hint">
                {balances.btc_pending_sats > 0 ? "+" : ""}
                {(balances.btc_pending_sats / 1e8).toFixed(8)} tBTC σε εκκρεμότητα (αναμονή επιβεβαίωσης)
              </p>
            )}
            <SendForm
              label="BTC"
              unit="tBTC"
              decimals={8}
              explorerBase="https://blockstream.info/testnet/tx/"
              onSend={(to, sats) => sendBtc(to, sats)}
              onSent={load}
            />
          </>
        ) : (
          <p>{loading ? "Φόρτωση..." : "—"}</p>
        )}
      </div>

      <div className="card">
        <h2>Solana (devnet)</h2>
        {addresses ? (
          <>
            <p className="address">{addresses.sol}</p>
            <CopyButton value={addresses.sol} />
            <p className="balance">
              {balances ? `${(balances.sol_lamports / 1e9).toFixed(4)} SOL` : loading ? "..." : "—"}
            </p>
            <p className="balance">
              {balances ? `${balances.stablecoin.toFixed(2)} ${balances.stablecoin_label}` : loading ? "..." : "—"}
            </p>
            <SendForm
              label="SOL"
              unit="SOL"
              decimals={9}
              explorerBase="https://explorer.solana.com/tx/"
              onSend={(to, lamports) => sendSol(to, lamports)}
              onSent={load}
            />
            <SendForm
              label={balances?.stablecoin_label ?? "USDC"}
              unit={balances?.stablecoin_label ?? "USDC"}
              decimals={6}
              explorerBase="https://explorer.solana.com/tx/"
              onSend={(to, base) => sendUsdc(to, base)}
              onSent={load}
            />
          </>
        ) : (
          <p>{loading ? "Φόρτωση..." : "—"}</p>
        )}
      </div>

      <div className="card">
        <h2>EVM (Polygon / Arbitrum)</h2>
        {addresses ? (
          <>
            <p className="address">{addresses.evm}</p>
            <CopyButton value={addresses.evm} />
            <p className="hint">
              Ίδια διεύθυνση για όλα τα EVM δίκτυα. Το USDC κάθε δικτύου είναι ξεχωριστό — πρόσεξε
              πάντα σε ποιο chain λαμβάνεις/στέλνεις.
            </p>
            {balances?.evm.map((b) => (
              <p className="balance" key={b.network}>
                {b.usdc.toFixed(2)} USDC <span className="chain-tag">{b.network}</span>
              </p>
            ))}
            {!balances && <p>{loading ? "Φόρτωση..." : "—"}</p>}
          </>
        ) : (
          <p>{loading ? "Φόρτωση..." : "—"}</p>
        )}
      </div>

      {error && <p className="error">{error}</p>}

      <button disabled={loading} onClick={load}>
        {loading ? "Φόρτωση..." : "Ανανέωση balances"}
      </button>
      <button disabled={busy} onClick={handleLock}>
        {busy ? "..." : "Κλείδωμα"}
      </button>
    </main>
  );
}
