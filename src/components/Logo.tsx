// App logo + wordmark. Swap `/amphora.svg` (in the public folder) with your
// own artwork — nothing else needs to change.
export default function Logo({ size = 40 }: { size?: number }) {
  return (
    <div className="logo">
      <img src="/amphora.svg" width={size} height={size * 1.5} alt="Amphoreus" />
      <span className="wordmark">Amphoreus</span>
    </div>
  );
}
