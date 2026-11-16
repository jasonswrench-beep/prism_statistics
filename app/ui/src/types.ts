// The shapes the engine sends back (crates/prism-api and crates/prism-engine/src/result.rs).

export type Measure = 'nominal' | 'ordinal' | 'scale';

export interface ValueLabel {
  value: number;
  label: string;
}

export interface VariableInfo {
  id: number;
  name: string;
  label: string;
  measure: Measure;
  type: 'numeric' | 'text';
  value_labels: ValueLabel[];
  missing_values: number[];
  /** Inclusive ranges of missing codes, from files that declare them (shown, not yet editable). */
  missing_ranges?: [number, number][];
  empty: number;
}

export interface DatasetInfo {
  name: string;
  rows: number;
  variables: VariableInfo[];
}

export interface ImportReport {
  rows: number;
  variables: number;
  renamed: [string, string][];
  short_rows: number;
  skipped_blank_rows: number;
  sheet: string | null;
  other_sheets: string[];
  notes: string[];
}

/** What opening a file that follows a known profile offers (an OpenAssess results file, for one). */
export interface ProfileOffer {
  about: string;
  suggestions: { dialog: string; label: string; args: Record<string, unknown> }[];
}

export interface ProjectState {
  path: string | null;
  name: string | null;
  dirty: boolean;
  has_data: boolean;
  extension: string;
  /** What Edit › Undo and Redo would do now, by name, or null. */
  undo?: string | null;
  redo?: string | null;
}

/** A Data View cell: a number, text, empty (null), or a user-missing code. */
export type CellValue = number | string | null | { missing: number };

export interface RowsPage {
  start: number;
  rows: CellValue[][];
}

export interface GroupCode {
  code: number;
  label: string | null;
  count: number;
}

export type ResultCell =
  | { kind: 'text'; text: string }
  | { kind: 'number'; value: number; decimals: number }
  | { kind: 'count'; value: number }
  | { kind: 'p'; value: number }
  | { kind: 'empty' };

export interface ResultTable {
  id: string;
  title: string;
  columns: string[];
  rows: ResultCell[][];
  footnotes: string[];
}

export type DiagnosticState = 'no_concern_detected' | 'review_needed' | 'unsupported_condition' | 'not_assessed';

export interface Diagnostic {
  id: string;
  title: string;
  state: DiagnosticState;
  message: string;
}

export interface ResultBundle {
  method_id: string;
  method_version: number;
  title: string;
  settings: unknown;
  status: { state: 'ok' } | { state: 'failed'; reason: string };
  cases: { total: number; analyzed: number; excluded: { reason: string; count: number }[] };
  tables: ResultTable[];
  diagnostics: Diagnostic[];
  method_note: string;
  fields: Record<string, number | string>;
  /** Figures from Graphs (SVG drawn by the engine, with alternative text). */
  charts?: { id: string; title: string; alt: string; svg: string }[];
}

export interface OutputItem {
  id: number;
  operation: number;
  bundle: ResultBundle;
  paragraph: string | null;
  stale: boolean;
  ai?: AiDraft | null;
  /** The command and arguments that made the result (to reopen its dialog with the same settings). */
  command: string;
  args: Record<string, unknown>;
}

export interface AiCheck {
  id: string;
  title: string;
  state: 'pass' | 'flag' | 'fail';
  message: string;
}

export interface AiDraft {
  text: string;
  raw: string;
  model: string;
  model_sha256: string;
  checks: AiCheck[];
  status: 'draft' | 'reviewed' | 'edited';
  seconds: number;
}

export interface AiStatus {
  enabled: boolean;
  installed: boolean;
  model: string;
  license: string;
  size: number;
  folder: string;
  downloading: boolean;
  done: number;
  total: number;
  error: string | null;
}

export interface TTestSpec {
  test_variables: string[];
  grouping_variable: string;
  groups: [number, number];
  estimator: 'welch' | 'student';
  level: number;
  alternative?: 'two_sided' | 'greater' | 'less';
  bayes?: boolean;
}
