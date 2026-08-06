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
  | "fold";

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
}
