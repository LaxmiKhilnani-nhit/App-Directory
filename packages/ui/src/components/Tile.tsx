import type { AppTile } from "@app-directory/types";

export interface TileProps {
  app: AppTile;
  onOpen: (app: AppTile) => void;
  disabled?: boolean;
}

const FALLBACK_COLORS = ["#2F6FED", "#0F766E", "#9A3412", "#6D28D9", "#B91C1C", "#0369A1"];

function fallbackColor(app: AppTile): string {
  if (app.color) return app.color;
  let hash = 0;
  for (const char of app.name) {
    hash = (hash * 31 + char.charCodeAt(0)) % 997;
  }
  return FALLBACK_COLORS[hash % FALLBACK_COLORS.length] ?? FALLBACK_COLORS[0]!;
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/).slice(0, 2);
  return words.map((word) => word.charAt(0).toUpperCase()).join("") || "?";
}

export function Tile({ app, onOpen, disabled = false }: TileProps) {
  return (
    <button
      type="button"
      className="ad-tile"
      onClick={() => onOpen(app)}
      disabled={disabled}
      title={app.description ?? app.name}
    >
      {app.iconUrl ? (
        <img className="ad-tile__icon" src={app.iconUrl} alt="" loading="lazy" />
      ) : (
        <span className="ad-tile__letter" style={{ background: fallbackColor(app) }}>
          {initials(app.name)}
        </span>
      )}
      <span className="ad-tile__body">
        <span className="ad-tile__name">{app.name}</span>
        <span className="ad-tile__meta">{app.category ?? app.description ?? ""}</span>
      </span>
    </button>
  );
}
