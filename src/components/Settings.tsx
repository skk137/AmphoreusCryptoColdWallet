import React from "react";

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
                                   onClose,
                                 }: {
  theme: string;
  setTheme: (t: string) => void;
  onClose: () => void;
}) {

  const [showPinChange, setShowPinChange] = React.useState(false);
  const [autoLock, setAutoLock] = React.useState(5);

  return (
      <div className="modal-overlay" onClick={onClose}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>

          <h1>Ρυθμίσεις</h1>


          {/* THEMES */}
          <h2>Θέμα</h2>

          <div className="theme-grid">

            {THEMES.map((t) => (
                <button
                    key={t.id}
                    data-theme={t.id}
                    className={
                      theme === t.id
                          ? "theme-opt active"
                          : "theme-opt"
                    }
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



          {/* SECURITY */}
          <h2>Ασφάλεια</h2>


          <div className="security-box">

            <button
                onClick={() => setShowPinChange(true)}
            >
              🔐 Αλλαγή PIN
            </button>


            <label>
              Αυτόματο κλείδωμα:
            </label>


            <select
                value={autoLock}
                onChange={(e) =>
                    setAutoLock(Number(e.target.value))
                }
            >

              {AUTO_LOCK_OPTIONS.map((o) => (
                  <option
                      key={o.value}
                      value={o.value}
                  >
                    {o.label}
                  </option>
              ))}

            </select>

          </div>


          {showPinChange && (
              <div className="pin-modal">

                <h3>Αλλαγή PIN</h3>

                <input
                    type="password"
                    placeholder="Τρέχον PIN"
                />

                <input
                    type="password"
                    placeholder="Νέο PIN"
                />

                <button
                    onClick={() => {
                      setShowPinChange(false);
                    }}
                >
                  Αποθήκευση
                </button>

              </div>
          )}



          <button onClick={onClose}>
            Κλείσιμο
          </button>


        </div>
      </div>
  );
}