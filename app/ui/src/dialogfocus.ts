// Keyboard behaviour shared by every dialog: the dialog takes the focus when it opens and gives it back when it
// closes; Tab and Shift+Tab stay inside it; and each list of variables is one tab stop, moved through with the arrow
// keys (Home and End go to the ends), as a list box is in any desktop program. List boxes outside dialogs (the
// docked panels) get the arrow keys and the single tab stop too.

const FOCUSABLE = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),summary,[tabindex]:not([tabindex="-1"])';

/** The elements Tab stops at, in order, skipping hidden ones and those inside closed sections. */
export function tabStops(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => {
    if (el.tabIndex < 0) return false;
    if (el.closest('[hidden],[inert]')) return false;
    const closed = el.closest('details:not([open])');
    if (closed && el.tagName !== 'SUMMARY') return false;
    if (closed && el.tagName === 'SUMMARY' && el.parentElement !== closed) return false;
    return el.getClientRects().length > 0;
  });
}

/** Options of a list box, in order. */
function options(list: Element): HTMLElement[] {
  return Array.from(list.querySelectorAll<HTMLElement>('[role="option"]')).filter((o) => o.closest('[role="listbox"]') === list);
}

/**
 * Makes one option of each list box the tab stop: the focused one, else the selected one, else the first.
 */
export function rovingStops(root: HTMLElement): void {
  for (const list of Array.from(root.querySelectorAll('[role="listbox"]'))) {
    const opts = options(list);
    if (!opts.length) continue;
    const active = root.ownerDocument.activeElement;
    const stop = opts.find((o) => o === active) ?? opts.find((o) => o.getAttribute('aria-selected') === 'true') ?? opts[0];
    for (const o of opts) {
      const want = o === stop ? '0' : '-1';
      if (o.getAttribute('tabindex') !== want) o.setAttribute('tabindex', want);
    }
  }
}

/** Arrow keys within a list box: the option to move to, or null when the key isn't one of them. */
export function arrowTarget(key: string, opts: HTMLElement[], current: HTMLElement): HTMLElement | null {
  const i = opts.indexOf(current);
  if (i < 0) return null;
  if (key === 'ArrowDown') return opts[Math.min(opts.length - 1, i + 1)];
  if (key === 'ArrowUp') return opts[Math.max(0, i - 1)];
  if (key === 'Home') return opts[0];
  if (key === 'End') return opts[opts.length - 1];
  return null;
}

/** Where the focus goes when a dialog opens: a control marked to start with, the first variable list, or the first control. */
function startAt(dialog: HTMLElement): HTMLElement | null {
  const marked = dialog.querySelector<HTMLElement>('[data-autofocus],[autofocus]');
  if (marked) return marked;
  const body = dialog.querySelector<HTMLElement>('.dbody') ?? dialog;
  const list = body.querySelector('[role="listbox"]');
  const first = list ? options(list)[0] : undefined;
  if (first) return first;
  // A dialog whose content is still loading holds the focus itself until it arrives (see watchDialogs).
  return tabStops(body)[0] ?? null;
}

interface Open {
  dialog: HTMLElement;
  before: HTMLElement | null;
}

/**
 * Gives every modal dialog in the document the keyboard behaviour above, as dialogs open and close. Called once when
 * the program starts, so each dialog gets it without doing anything itself. Returns a function that stops it.
 */
export function watchDialogs(doc: Document): () => void {
  const open: Open[] = [];
  const top = () => open[open.length - 1]?.dialog;

  const attach = (dialog: HTMLElement) => {
    if (open.some((o) => o.dialog === dialog)) return;
    const active = doc.activeElement as HTMLElement | null;
    // The element that had the focus before; a control inside the new dialog (autoFocus) means it was taken already.
    const before = active && !dialog.contains(active) ? active : (open[open.length - 1]?.dialog.querySelector<HTMLElement>(':focus-within') ?? null);
    open.push({ dialog, before: before && before !== doc.body ? before : null });
    if (!dialog.hasAttribute('tabindex')) dialog.setAttribute('tabindex', '-1');
    rovingStops(dialog);
    if (!dialog.contains(doc.activeElement)) (startAt(dialog) ?? dialog).focus();
  };

  const detachGone = () => {
    for (let i = open.length - 1; i >= 0; i--) {
      if (open[i].dialog.isConnected) continue;
      const [gone] = open.splice(i, 1);
      const now = doc.activeElement;
      // Back to where the focus was, unless something else (a new result, another dialog) has taken it since.
      if (now && now !== doc.body && now.isConnected) continue;
      const back = gone.before && gone.before.isConnected ? gone.before : null;
      const under = top();
      if (back && (!under || under.contains(back))) back.focus();
      else if (under) (startAt(under) ?? under).focus();
    }
  };

  // The last option that had the focus, so when it leaves its list (moved to a role box) the focus stays in the list.
  let lastOption: { list: Element; index: number } | null = null;

  const scan = () => {
    const lost = doc.activeElement === doc.body || doc.activeElement === null;
    if (lost && lastOption && lastOption.list.isConnected) {
      const opts = options(lastOption.list);
      const to = opts[Math.min(lastOption.index, opts.length - 1)];
      if (to) to.focus();
      else (lastOption.list.closest<HTMLElement>('[role="dialog"]') ?? doc.body).focus();
    }
    detachGone();
    for (const d of Array.from(doc.querySelectorAll<HTMLElement>('[role="dialog"][aria-modal="true"]'))) attach(d);
    const t = top();
    if (t) {
      rovingStops(t);
      // Content that arrived after the dialog opened: the focus, still on the dialog itself, moves to it.
      if (doc.activeElement === t) startAt(t)?.focus();
    }
  };

  const observer = new MutationObserver(scan);
  observer.observe(doc.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['aria-selected'] });

  const onKey = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement;
    const dialog = top();
    if (e.key === 'Tab' && dialog) {
      const stops = tabStops(dialog);
      if (!stops.length) {
        e.preventDefault();
        return;
      }
      const first = stops[0];
      const last = stops[stops.length - 1];
      const inside = dialog.contains(target) && target !== dialog;
      if (e.shiftKey && (!inside || target === first)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (!inside || target === last)) {
        e.preventDefault();
        first.focus();
      }
      return;
    }
    if (target.getAttribute?.('role') === 'option') {
      const list = target.closest('[role="listbox"]');
      if (list) lastOption = { list, index: options(list).indexOf(target) };
      const to = list ? arrowTarget(e.key, options(list), target) : null;
      if (to) {
        e.preventDefault();
        to.focus();
        lastOption = { list: list!, index: options(list!).indexOf(to) };
        rovingStops(list!.parentElement ?? (list as HTMLElement));
      }
    }
  };
  const onFocusIn = (e: FocusEvent) => {
    const dialog = top();
    const target = e.target as HTMLElement;
    // The focus never stays behind an open dialog.
    if (dialog && !dialog.contains(target)) {
      (startAt(dialog) ?? dialog).focus();
      return;
    }
    const list = target.closest?.('[role="listbox"]');
    if (list) rovingStops(list.parentElement ?? (list as HTMLElement));
    lastOption = list && target.getAttribute('role') === 'option' ? { list, index: options(list).indexOf(target) } : null;
  };
  doc.addEventListener('keydown', onKey);
  doc.addEventListener('focusin', onFocusIn);
  scan();
  return () => {
    observer.disconnect();
    doc.removeEventListener('keydown', onKey);
    doc.removeEventListener('focusin', onFocusIn);
  };
}
