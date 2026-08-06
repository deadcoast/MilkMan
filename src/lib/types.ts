export type BottleState =
  | "inspecting"
  | "bottling"
  | "chilling"
  | "delivering"
  | "fresh"
  | "spoiled"
  | "capped";

export type LineKind =
  | "stage"
  | "stagedone"
  | "tree"
  | "ok"
  | "warn"
  | "err"
  | "note"
  | "raw"
  | "url"
  | "fold"
  | "sig"
  | "cmd";

export interface Line {
  kind: LineKind;
  text: string;
}

export interface Profile {
  stack: string;
  glyph: string;
  manager: string;
  framework: string | null;
  confidence: string;
  /** package.json script names (node family only) */
  scripts: string[];
}

export interface BottleView {
  id: number;
  name: string;
  path: string;
  state: BottleState;
  profile: Profile | null;
  url: string | null;
  port: number | null;
  lines: Line[];
  /** full uncapped raw log, toggled with L */
  rawLog: string[];
  foldedCount: number;
  startedAt: number;
  deliveredAt: number | null;
  /** true for bottles brought back from the ledger, until re-poured */
  restored: boolean;
  /** where the previous delivery was served (from the ledger) */
  lastUrl: string | null;
  /** last-log replay already fetched (guards the lazy load) */
  historyLoaded: boolean;
}

export interface LinePayload {
  bottle: number;
  kind: LineKind;
  text: string;
  rewrite: boolean;
}

export interface StatePayload {
  bottle: number;
  state: BottleState;
  url: string | null;
  port: number | null;
  profile: Profile | null;
}

export interface BottleInfo {
  id: number;
  name: string;
  path: string;
  state: BottleState;
  stack: string | null;
  glyph: string | null;
  manager: string | null;
  framework: string | null;
  last_url: string | null;
  restored: boolean;
}

export interface LogLine {
  kind: string;
  text: string;
}
