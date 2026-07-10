export const THEMES = [
  { id: "amphora", label: "Amphora (default)" },
  { id: "purple", label: "Βαθύ μωβ" },
  { id: "ocean", label: "Ωκεανός" },
  { id: "light", label: "Ανοιχτό" },
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

        <p className="hint">Αλλαγή PIN και χρόνος auto-lock έρχονται εδώ σύντομα.</p>
        <button onClick={onClose}>Κλείσιμο</button>
      </div>
    </div>
  );
}
