// The one door between the interface and the engine.
//
// Inside the Tauri window it uses invoke('call', { cmd, args }). In a browser it posts the arguments to /api/<cmd>,
// which `prism serve` answers. Both reach the same command layer (crates/prism-api), so what is tested in a browser
// is what ships. A failure always throws an Error whose message can be shown to the user as it is.

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Commands waiting for an answer, so a long analysis can show its progress and a Cancel button (JobProgress).
let pending = 0;
const watchers = new Set<(n: number) => void>();
export function watchPending(f: (n: number) => void): () => void {
  watchers.add(f);
  return () => watchers.delete(f);
}
const QUIET = new Set(['job_status', 'cancel_job', 'autosave']);

export async function call<T>(cmd: string, args: unknown = {}): Promise<T> {
  if (QUIET.has(cmd)) return send<T>(cmd, args);
  pending += 1;
  watchers.forEach((f) => f(pending));
  try {
    return await send<T>(cmd, args);
  } finally {
    pending -= 1;
    watchers.forEach((f) => f(pending));
  }
}

async function send<T>(cmd: string, args: unknown): Promise<T> {
  if (inTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    try {
      return (await invoke('call', { cmd, args })) as T;
    } catch (e) {
      throw new Error(typeof e === 'string' ? e : String((e as { message?: string })?.message ?? e));
    }
  }
  let response: Response;
  try {
    // The test server runs commands only for its own page, which it serves with a token (prism serve).
    const token = document.querySelector<HTMLMetaElement>('meta[name="prism-token"]')?.content ?? '';
    response = await fetch(`/api/${cmd}`, { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Prism-Token': token }, body: JSON.stringify(args) });
  } catch {
    throw new Error('The engine is not running. Start it with: prism serve');
  }
  const body = (await response.json()) as { ok?: T; error?: string };
  if (body.error !== undefined) throw new Error(body.error);
  return body.ok as T;
}
