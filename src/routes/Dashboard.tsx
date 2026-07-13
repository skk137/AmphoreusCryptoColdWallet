import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import Logo from "../components/Logo";
import { useT } from "../lib/i18n";
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
  sendLtc,
  sendEvm,
  sendSol,
  sendUsdc,
  walletSourcePresent,
} from "../lib/tauri";

// Auto-lock after this much inactivity. Any mouse/keyboard activity resets it.
function CopyButton({ value }: { value: string }) {
  const { t } = useT();
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
      {copied ? t("copied") : t("copy")}
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
  const { t } = useT();
  const [showQr, setShowQr] = useState(false);
  return (
    <div>
      <p className="address">{value}</p>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <CopyButton value={value} />
        <button className="copy" onClick={() => setShowQr((v) => !v)}>
          {showQr ? t("hide_qr") : t("qr")}
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
  const { t } = useT();
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
      setError(t("fill_recipient"));
      return;
    }
    if (!parsed || parsed <= 0) {
      setError(t("invalid_amount"));
      return;
    }
    const baseUnits = Math.round(parsed * Math.pow(10, decimals));

    // Fee preview before the confirmation dialog.
    let feeLine = "";
    if (estimateFee) {
      setEstimating(true);
      try {
        feeLine = `\n${t("est_fee", await estimateFee(to.trim(), baseUnits))}\n`;
      } catch (e) {
        feeLine = `\n${t("fee_failed", String(e))}\n`;
      } finally {
        setEstimating(false);
      }
    }

    const ok = window.confirm(t("confirm_send", parsed, unit, to.trim(), feeLine));
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
        {t("send_x", label)}
      </button>
    );
  }

  return (
    <div className="send-form">
      <input placeholder={t("recipient_ph")} value={to} onChange={(e) => setTo(e.target.value)} />
      <input
        placeholder={t("amount_in", unit)}
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
      />
      <button disabled={sending || estimating} onClick={handleSend}>
        {estimating ? t("calc_fee") : sending ? t("sending") : t("send_x", unit)}
      </button>
      <button className="copy" onClick={() => setOpen(false)}>
        {t("cancel")}
      </button>
      {result && (
        <p className="hint">
          {t("sent_txid")} <span className="address">{result}</span>
          <br />
          <a href={`${explorerBase}${result}`} target="_blank" rel="noreferrer">
            {t("view_tx")}
          </a>
        </p>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

function EvmSendForm({ b, onSent }: { b: EvmBalance; onSent: () => void }) {
  const { t } = useT();
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
    if (!to.trim()) return setError(t("fill_recipient"));
    if (!amt || parseFloat(amt) <= 0) return setError(t("invalid_amount"));
    const ok = window.confirm(
      t("confirm_send_evm", amt, assetLabel, b.network, to.trim(), b.native_symbol)
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
        {t("send_on_net", b.network)}
      </button>
    );
  }

  return (
    <div className="send-form">
      <select value={asset} onChange={(e) => setAsset(e.target.value)}>
        <option value="native">{b.native_symbol} (native)</option>
        {b.tokens.map((tok) => (
          <option key={tok.symbol} value={tok.symbol}>
            {tok.symbol}
          </option>
        ))}
      </select>
      <input placeholder={t("recipient_ph_evm")} value={to} onChange={(e) => setTo(e.target.value)} />
      <input placeholder={t("amount_in", assetLabel)} value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button disabled={sending} onClick={handleSend}>
        {sending ? t("sending") : t("send_x", assetLabel)}
      </button>
      <button className="copy" onClick={() => setOpen(false)}>
        {t("cancel")}
      </button>
      {result && (
        <p className="hint">
          {t("sent_txid")} <span className="address">{result}</span>
          <br />
          <a href={`${b.explorer_tx}${result}`} target="_blank" rel="noreferrer">
            {t("view_tx")}
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
  const { t } = useT();
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
    s === "confirmed" ? t("st_confirmed") : s === "failed" ? t("st_failed") : t("st_pending");

  return (
    <div className="card">
      <h2>{t("tx_history")}</h2>
      <button className="copy" onClick={load} disabled={loading}>
        {loading ? t("loading") : t("refresh")}
      </button>
      {txs && txs.length === 0 && <p className="hint">{t("no_tx")}</p>}
      {txs?.map((tx) => (
        <div key={tx.txid} className="history-row">
          <span className="chain-tag">{tx.chain}</span>
          <span className={`hist-status ${tx.status}`}>{statusLabel(tx.status)}</span>
          <a
            href={tx.explorer_url}
            target="_blank"
            rel="noreferrer"
            className="address"
            style={{ fontSize: "0.7rem", margin: 0 }}
          >
            {tx.txid.slice(0, 18)}…
          </a>
        </div>
      ))}
      <p className="hint" style={{ marginTop: "0.7rem" }}>
        {t("full_evm_history")}{" "}
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

export default function Dashboard({
  onLocked,
  autoLockMin,
}: {
  onLocked: () => void;
  autoLockMin: number;
}) {
  const { t } = useT();
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
      setBalances(await getBalances(addrs.btc, addrs.sol, addrs.evm, addrs.ltc, addrs.doge));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, []);

  // Auto-lock: idle timeout (configurable; 0 = never) + USB removal.
  // Re-runs whenever autoLockMin changes so the new timeout takes effect.
  useEffect(() => {
    const lockMs = autoLockMin * 60 * 1000; // 0 = idle-lock disabled
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
      setLockProgress(lockMs > 0 ? Math.min(1, elapsed / lockMs) : 0);
      if (lockMs > 0 && elapsed >= lockMs) {
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
  }, [autoLockMin]);

  async function handleLock() {
    setBusy(true);
    try {
      await lockWallet();
    } finally {
      setBusy(false);
      onLocked();
    }
  }

  const lockSecondsLeft = Math.max(0, Math.ceil((autoLockMin * 60 * 1000 * (1 - lockProgress)) / 1000));
  const lockMin = Math.floor(lockSecondsLeft / 60);
  const lockSec = String(lockSecondsLeft % 60).padStart(2, "0");

  return (
    <main className="container">
      <Logo size={32} />

      <nav className="tabs">
        <button className={tab === "home" ? "tab active" : "tab"} onClick={() => setTab("home")}>
          {t("home")}
        </button>
        <button
          className={tab === "history" ? "tab active" : "tab"}
          onClick={() => setTab("history")}
        >
          {t("tx_history_tab")}
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
          <p className="hint">{t("loading")}</p>
        )
      ) : (
        <>
      {addresses && <p className="badge">{addresses.network} — {t("testnets_badge")}</p>}

      <div className="card">
        <h2>{t("card_btc")}</h2>
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
                {t("btc_pending", (balances.btc_pending_sats / 1e8).toFixed(8))}
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
          <p>{loading ? t("loading") : t("dash")}</p>
        )}
      </div>

      <div className="card">
        <h2>{t("card_sol")}</h2>
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
          <p>{loading ? t("loading") : t("dash")}</p>
        )}
      </div>

      <div className="card">
        <h2>{t("card_evm")}</h2>
        {addresses ? (
          <>
            <AddressView value={addresses.evm} />
            <p className="hint">{t("evm_same_addr")}</p>
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
                          {t("native_fees", b.native_symbol)}
                        </p>
                        <EvmSendForm b={b} onSent={load} />
                      </div>
                    )}
                  </>
                );
              })()
            ) : (
              <p>{loading ? t("loading") : t("dash")}</p>
            )}
          </>
        ) : (
          <p>{loading ? t("loading") : t("dash")}</p>
        )}
      </div>

      <div className="card">
        <h2>{t("card_ltc")}</h2>
        {addresses ? (
          <>
            <AddressView value={addresses.ltc} />
            <p className="balance">
              <CoinIcon symbol="LTC" />
              {balances ? `${(balances.ltc_sats / 1e8).toFixed(8)} tLTC` : loading ? "..." : "—"}
            </p>
            <SendForm
              label="LTC"
              unit="tLTC"
              decimals={8}
              explorerBase="https://litecoinspace.org/testnet/tx/"
              onSend={(to, sats) => sendLtc(to, sats)}
              onSent={load}
            />
          </>
        ) : (
          <p>{loading ? t("loading") : t("dash")}</p>
        )}
      </div>

      <div className="card">
        <h2>{t("card_doge")}</h2>
        {addresses ? (
          <>
            <AddressView value={addresses.doge} />
            <p className="balance">
              <CoinIcon symbol="DOGE" />
              {balances ? `${(balances.doge_koinu / 1e8).toFixed(4)} DOGE` : loading ? "..." : "—"}
            </p>
            <p className="hint">{t("doge_note")}</p>
          </>
        ) : (
          <p>{loading ? t("loading") : t("dash")}</p>
        )}
      </div>

      {error && <p className="error">{error}</p>}

      <button disabled={loading} onClick={load}>
        {loading ? t("loading") : t("refresh_balances")}
      </button>
        </>
      )}

      <button disabled={busy} onClick={handleLock}>
        {busy ? "..." : t("lock")}
      </button>

      {autoLockMin > 0 && (
        <div className="autolock" title={t("lock_in", lockMin, lockSec)}>
          <span className="autolock-label">{t("lock_in", lockMin, lockSec)}</span>
          <div className="autolock-bar">
            <div className="autolock-fill" style={{ width: `${lockProgress * 100}%` }} />
          </div>
        </div>
      )}
    </main>
  );
}
