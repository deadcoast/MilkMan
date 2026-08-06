// Tiny persisted settings — localStorage-backed, read once at startup.

const AUTO_OPEN_KEY = "milkman.autoOpen";
const CELL_KEY = "milkman.cell";

export const CELL_BASE = 11;
export const CELL_MIN = 8;
export const CELL_MAX = 18;

class Settings {
  /** open every FRESH delivery in the browser automatically */
  autoOpen = localStorage.getItem(AUTO_OPEN_KEY) === "1";
  /** terminal cell size in px (ctrl +/− zoom) */
  cell = clampCell(Number(localStorage.getItem(CELL_KEY)) || CELL_BASE);

  setAutoOpen(on: boolean) {
    this.autoOpen = on;
    localStorage.setItem(AUTO_OPEN_KEY, on ? "1" : "0");
  }

  setCell(px: number) {
    this.cell = clampCell(px);
    localStorage.setItem(CELL_KEY, String(this.cell));
    applyCell(this.cell);
  }
}

function clampCell(px: number): number {
  return Math.min(CELL_MAX, Math.max(CELL_MIN, Math.round(px)));
}

export function applyCell(px: number): void {
  document.documentElement.style.setProperty("--cell", `${px}px`);
}

export const settings = new Settings();
