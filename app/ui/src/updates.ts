// Help › Check for Updates: the pieces of the dialog that need no window (components/UpdatesDialog.tsx).

/** How far a download has got, as a whole percent, or null when the size isn't known. */
export function progressPercent(received: number, total: number | null | undefined): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.floor((100 * received) / total));
}

/** Whether the once-a-day check is due: turned on, and never checked or last checked a day or more ago. */
export function dailyCheckDue(on: boolean, lastCheck: string, now: Date = new Date()): boolean {
  if (!on) return false;
  const last = Date.parse(lastCheck);
  return Number.isNaN(last) || now.getTime() - last >= 24 * 60 * 60 * 1000;
}
