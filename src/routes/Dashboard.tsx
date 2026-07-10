import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import Logo from "../components/Logo";
import {
  Addresses,
  Balances,
  EvmBalance,
  HistoryTx,
  estimateBtcFee,
  getAddresses,
  getBalances,
  getHistory,
  lockWallet,
  sendBtc,
  sendEvm,
  sendSol,
  sendUsdc,
  walletSourcePresent,
} from "../lib/tauri";

// Auto-lock after this much inactivity. Any mouse/keyboard activity resets it.
const AUTO_LOCK_MS = 15 * 60 * 1000;

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

function CoinIcon({ symbol, size = 18 }: { symbol: string; size?: number }) {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return (
      <span
        className="coin-fallback"
        style={{ width: size, height: size, fontSize: size * 0.5 }}
        aria-hidden
      >
        {symbol.slice(0, 1)}
      </span>
    );
  }
  return (
    <img
      className="coin-icon"
      src={`/coins/${symbol.toLowerCase()}.svg`}
      width={size}
      height={size}
      alt=""
      onError={() => setFailed(true)}
    />
  );
}

function AddressView({ value }: { value: string }) {
  const [showQr, setShowQr] = useState(false);
  return (
    <div>
      <p className="address">{value}</p>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <CopyButton value={value} />
        <button className="copy" onClick={() => setShowQr((v) => !v)}>
          {showQr ? "Κρύψε QR" : "QR"}
        </button>
      </div>
      {showQr && (
        <div className="qr">
          <QRCodeSVG value={value} size={140} bgColor="#ffffff" fgColor="#111111" level="M" />
        </div>
      )}
    </div>
  );
}

function SendForm({
  label,
  unit,
  decimals,
  explorerBase,
  estimateFee,
  onSend,
  onSent,
}: {
  label: string;
  unit: string;
  decimals: number;
  explorerBase: string;
  estimateFee?: (to: string, baseUnits: number) => Promise<string>;
  onSend: (to: string, baseUnits: number) => Promise<string>;
  onSent: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const [sending, setSending] = useState(false);
  const [estimating, setEstimating] = useState(false);
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

    // Fee preview before the confirmation dialog.
    let feeLine = "";
    if (estimateFee) {
      setEstimating(true);
      try {
        feeLine = `\nΕκτιμώμενο fee: ${await estimateFee(to.trim(), baseUnits)}\n`;
      } catch (e) {
        feeLine = `\n(δεν υπολογίστηκε το fee: ${e})\n`;
      } finally {
        setEstimating(false);
      }
    }

    const ok = window.confirm(
      `Επιβεβαίωση αποστολής:\n\n${parsed} ${unit}\nπρος: ${to.trim()}\n${feeLine}\nΗ συναλλαγή δεν μπορεί να ανακληθεί. Συνέχεια;`
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
      <button disabled={sending || estimating} onClick={handleSend}>
        {estimating ? "Υπολογισμός fee..." : sending ? "Αποστολή..." : `Αποστολή ${unit}`}
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

function EvmSendForm({ b, onSent }: { b: EvmBalance; onSent: () => void }) {
  const [open, setOpen] = useState(false);
  const [asset, setAsset] = useState("native"); // "native" | token symbol
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const [sending, setSending] = useState(false);
  const [result, setResult] = useState("");
  const [error, setError] = useState("");

  const assetLabel = asset === "native" ? b.native_symbol : asset;

  async function handleSend() {
    setError("");
    setResult("");
    const amt = amount.trim().replace(",", ".");
    if (!to.trim()) return setError("Συμπλήρωσε τη διεύθυνση παραλήπτη.");
    if (!amt || parseFloat(amt) <= 0) return setError("Μη έγκυρο ποσό.");
    const ok = window.confirm(
      `Επιβεβαίωση αποστολής:\n\n${amt} ${assetLabel}\nδίκτυο: ${b.network}\nπρος: ${to.trim()}\n\nΤο fee πληρώνεται σε ${b.native_symbol} (gas).\nΗ συναλλαγή δεν ανακαλείται. Συνέχεια;`
    );
    if (!ok) return;
    setSending(true);
    try {
      const hash = await sendEvm(b.network, asset, to.trim(), amt);
      setResult(hash);
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
        Αποστολή ({b.network})
      </button>
    );
  }

  return (
    <div className="send-form">
      <select value={asset} onChange={(e) => setAsset(e.target.value)}>
        <option value="native">{b.native_symbol} (native)</option>
        {b.tokens.map((t) => (
          <option key={t.symbol} value={t.symbol}>
            {t.symbol}
          </option>
        ))}
      </select>
      <input placeholder="Διεύθυνση παραλήπτη (0x...)" value={to} onChange={(e) => setTo(e.target.value)} />
      <input placeholder={`Ποσό σε ${assetLabel}`} value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button disabled={sending} onClick={handleSend}>
        {sending ? "Αποστολή..." : `Αποστολή ${assetLabel}`}
      </button>
      <button className="copy" onClick={() => setOpen(false)}>
        Άκυρο
      </button>
      {result && (
        <p className="hint">
          Εστάλη! <span className="address">{result}</span>
          <br />
          <a href={`${b.explorer_tx}${result}`} target="_blank" rel="noreferrer">
            Προβολή στο explorer
          </a>
        </p>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

function HistoryCard({
  btc,
  sol,
  evm,
  evmBalances,
}: {
  btc: string;
  sol: string;
  evm: string;
  evmBalances: EvmBalance[];
}) {
  const [txs, setTxs] = useState<HistoryTx[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  async function load() {
    setLoading(true);
    setError("");
    try {
      setTxs(await getHistory(btc, sol));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, []);

  const statusLabel = (s: string) =>
    s === "confirmed" ? "✓ επιβεβαιωμένη" : s === "failed" ? "✗ απέτυχε" : "… εκκρεμεί";

  return (
    <div className="card">
      <h2>Ιστορικό συναλλαγών</h2>
      <button className="copy" onClick={load} disabled={loading}>
        {loading ? "Φόρτωση..." : "Ανανέωση"}
      </button>
      {txs && txs.length === 0 && <p className="hint">Καμία συναλλαγή ακόμα.</p>}
      {txs?.map((t) => (
        <div key={t.txid} className="history-row">
          <span className="chain-tag">{t.chain}</span>
          <span className={`hist-status ${t.status}`}>{statusLabel(t.status)}</span>
          <a
            href={t.explorer_url}
            target="_blank"
            rel="noreferrer"
            className="address"
            style={{ fontSize: "0.7rem", margin: 0 }}
          >
            {t.txid.slice(0, 18)}…
          </a>
        </div>
      ))}
      <p className="hint" style={{ marginTop: "0.7rem" }}>
        Πλήρες EVM ιστορικό στο explorer:{" "}
        {evmBalances.map((b) => (
          <a
            key={b.network}
            href={`${b.explorer_tx.replace("/tx/", "/address/")}${evm}`}
            target="_blank"
            rel="noreferrer"
            style={{ marginRight: "0.6rem" }}
          >
            {b.network.split(" ")[0]}
          </a>
        ))}
      </p>
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
  const [lockProgress, setLockProgress] = useState(0); // 0 → 1 (full = lock)
  const [tab, setTab] = useState<"home" | "history">("home");
  const [evmNet, setEvmNet] = useState(""); // selected EVM network name ("" = first)

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

  // Auto-lock: idle timeout (bar fills over 15 min of inactivity) + USB removal.
  useEffect(() => {
    let lastActivity = Date.now();
    let locked = false;
    const bump = () => (lastActivity = Date.now());
    const events = ["mousemove", "mousedown", "keydown", "scroll", "touchstart"];
    events.forEach((e) => window.addEventListener(e, bump));

    const lockNow = async () => {
      if (locked) return;
      locked = true;
      await lockWallet();
      onLocked();
    };

    let tick = 0;
    const interval = setInterval(async () => {
      const elapsed = Date.now() - lastActivity;
      setLockProgress(Math.min(1, elapsed / AUTO_LOCK_MS));
      if (elapsed >= AUTO_LOCK_MS) {
        await lockNow();
        return;
      }
      // Check every ~3s whether the USB/source is still plugged in.
      if (tick++ % 3 === 0) {
        try {
          if (!(await walletSourcePresent())) await lockNow();
        } catch {
          /* transient — ignore */
        }
      }
    }, 1000);

    return () => {
      clearInterval(interval);
      events.forEach((e) => window.removeEventListener(e, bump));
    };
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

  const lockSecondsLeft = Math.max(0, Math.ceil((AUTO_LOCK_MS * (1 - lockProgress)) / 1000));
  const lockMin = Math.floor(lockSecondsLeft / 60);
  const lockSec = String(lockSecondsLeft % 60).padStart(2, "0");

  return (
    <main className="container">
      <Logo size={32} />

      <nav className="tabs">
        <button className={tab === "home" ? "tab active" : "tab"} onClick={() => setTab("home")}>
          Home
        </button>
        <button
          className={tab === "history" ? "tab active" : "tab"}
          onClick={() => setTab("history")}
        >
          Transaction History
        </button>
      </nav>

      {tab === "history" ? (
        addresses && balances ? (
          <HistoryCard
            btc={addresses.btc}
            sol={addresses.sol}
            evm={addresses.evm}
            evmBalances={balances.evm}
          />
        ) : (
          <p className="hint">Φόρτωση...</p>
        )
      ) : (
        <>
      {addresses && <p className="badge">{addresses.network} — δοκιμαστικά δίκτυα </p>}

      <div className="card">
        <h2>Bitcoin (testnet)</h2>
        {addresses ? (
          <>
            <AddressView value={addresses.btc} />
            <p className="balance">
              <CoinIcon symbol="BTC" />
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
              estimateFee={async (_to, sats) => {
                const e = await estimateBtcFee(sats);
                return `${(e.fee_sats / 1e8).toFixed(8)} tBTC · σύνολο ${(e.total_sats / 1e8).toFixed(8)} tBTC`;
              }}
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
            <AddressView value={addresses.sol} />
            <p className="balance">
              <CoinIcon symbol="SOL" />
              {balances ? `${(balances.sol_lamports / 1e9).toFixed(4)} SOL` : loading ? "..." : "—"}
            </p>
            <p className="balance">
              <CoinIcon symbol={balances?.stablecoin_label ?? "USDC"} />
              {balances ? `${balances.stablecoin.toFixed(2)} ${balances.stablecoin_label}` : loading ? "..." : "—"}
            </p>
            <SendForm
              label="SOL"
              unit="SOL"
              decimals={9}
              explorerBase="https://explorer.solana.com/tx/"
              estimateFee={async () => "~0.000005 SOL (fee δικτύου)"}
              onSend={(to, lamports) => sendSol(to, lamports)}
              onSent={load}
            />
            <SendForm
              label={balances?.stablecoin_label ?? "USDC"}
              unit={balances?.stablecoin_label ?? "USDC"}
              decimals={6}
              explorerBase="https://explorer.solana.com/tx/"
              estimateFee={async () =>
                "~0.000005 SOL + ~0.002 SOL αν ο παραλήπτης δεν έχει ήδη λογαριασμό USDC"
              }
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
            <AddressView value={addresses.evm} />
            <p className="hint">
              Ίδια διεύθυνση για όλα τα EVM δίκτυα. Το USDC κάθε δικτύου είναι ξεχωριστό — πρόσεξε
              πάντα σε ποιο chain λαμβάνεις/στέλνεις.
            </p>
            {balances ? (
              (() => {
                const selectedNet = evmNet || balances.evm[0]?.network || "";
                const b = balances.evm.find((x) => x.network === selectedNet) ?? balances.evm[0];
                return (
                  <>
                    <select
                      className="net-select"
                      value={selectedNet}
                      onChange={(e) => setEvmNet(e.target.value)}
                    >
                      {balances.evm.map((n) => (
                        <option key={n.network} value={n.network}>
                          {n.network}
                        </option>
                      ))}
                    </select>
                    {b && (
                      <div className="evm-net">
                        {b.tokens.map((t) => (
                          <p className="balance" key={t.symbol} style={{ margin: "0 0 0.4rem" }}>
                            <CoinIcon symbol={t.symbol} />
                            {t.amount.toFixed(2)} {t.symbol}{" "}
                            <span className="chain-tag">{b.network}</span>
                          </p>
                        ))}
                        <p className="balance" style={{ margin: 0 }}>
                          <CoinIcon symbol={b.native_symbol} />
                          {b.native.toFixed(4)} {b.native_symbol}{" "}
                          <span className="chain-tag">{b.network}</span>
                        </p>
                        <p className="hint" style={{ margin: "0.1rem 0 0" }}>
                          {b.native_symbol} = native νόμισμα, πληρώνει τα fees
                        </p>
                        <EvmSendForm b={b} onSent={load} />
                      </div>
                    )}
                  </>
                );
              })()
            ) : (
              <p>{loading ? "Φόρτωση..." : "—"}</p>
            )}
          </>
        ) : (
          <p>{loading ? "Φόρτωση..." : "—"}</p>
        )}
      </div>

      {error && <p className="error">{error}</p>}

      <button disabled={loading} onClick={load}>
        {loading ? "Φόρτωση..." : "Ανανέωση balances"}
      </button>
        </>
      )}

      <button disabled={busy} onClick={handleLock}>
        {busy ? "..." : "Κλείδωμα"}
      </button>

      <div className="autolock" title={`Αυτόματο κλείδωμα σε ${lockMin}:${lockSec}`}>
        <span className="autolock-label">κλείδωμα σε {lockMin}:{lockSec}</span>
        <div className="autolock-bar">
          <div className="autolock-fill" style={{ width: `${lockProgress * 100}%` }} />
        </div>
      </div>
    </main>
  );
}
