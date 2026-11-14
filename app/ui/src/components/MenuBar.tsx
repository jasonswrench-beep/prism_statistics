// The classic menu bar (FAM-01): File, Edit, View, Data, Transform, Analyze, Graphs, Results, Tools, Help.
// Menus open on click or Enter, move with the arrow keys, and close with Escape. Items that aren't built yet are shown
// as "Coming in a later release" so the map stays familiar while the app grows.

import { useEffect, useRef, useState } from 'react';

export interface MenuItem {
  label: string;
  shortcut?: string;
  onSelect?: () => void;
  disabled?: boolean;
  later?: boolean;
  submenu?: MenuItem[];
}

export type MenuEntry = MenuItem | '-';

export interface Menu {
  label: string;
  items: MenuEntry[];
}

export function MenuBar({ menus }: { menus: Menu[] }) {
  const [open, setOpen] = useState<number | null>(null);
  const [sub, setSub] = useState<string | null>(null);
  const barRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) setOpen(null);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, []);

  useEffect(() => {
    if (open === null) return;
    const first = barRef.current?.querySelector<HTMLElement>(`[data-menu="${open}"] [role=menuitem]:not([aria-disabled=true])`);
    first?.focus();
  }, [open]);

  const choose = (item: MenuItem) => {
    if (item.disabled || item.later || !item.onSelect) return;
    setOpen(null);
    setSub(null);
    item.onSelect();
  };

  const onMenuKey = (e: React.KeyboardEvent<HTMLDivElement>, index: number) => {
    const items = Array.from(e.currentTarget.querySelectorAll<HTMLElement>(':scope > [role=menuitem], :scope > .sub > [role=menuitem]'));
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const next = (at + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length;
      items[next]?.focus();
    } else if (e.key === 'Escape') {
      setOpen(null);
      barRef.current?.querySelector<HTMLElement>(`[data-top="${index}"]`)?.focus();
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      const next = (index + (e.key === 'ArrowRight' ? 1 : menus.length - 1)) % menus.length;
      setOpen(next);
    }
  };

  const renderItems = (entries: MenuEntry[], path: string) =>
    entries.map((entry, i) => {
      if (entry === '-') return <hr key={`${path}-${i}`} />;
      const key = `${path}-${entry.label}`;
      if (entry.submenu) {
        return (
          <div className="sub" key={key} onMouseEnter={() => setSub(key)} onMouseLeave={() => setSub(null)}>
            <button type="button" role="menuitem" aria-haspopup="menu" aria-expanded={sub === key} onClick={() => setSub(sub === key ? null : key)} onKeyDown={(e) => e.key === 'ArrowRight' && (e.stopPropagation(), setSub(key))}>
              {entry.label}
              <span className="kbd">›</span>
            </button>
            <div className={`menu ${sub === key ? 'open' : ''}`} role="menu" aria-label={entry.label} onKeyDown={(e) => e.key === 'ArrowLeft' && (e.stopPropagation(), setSub(null))}>
              {renderItems(entry.submenu, key)}
            </div>
          </div>
        );
      }
      const inactive = entry.disabled || entry.later || !entry.onSelect;
      return (
        <button type="button" role="menuitem" key={key} aria-disabled={inactive || undefined} onClick={() => choose(entry)}>
          {entry.label}
          {entry.later ? <span className="later">Later release</span> : entry.shortcut ? <span className="kbd">{entry.shortcut}</span> : null}
        </button>
      );
    });

  return (
    <nav className="menubar" role="menubar" aria-label="Main menu" ref={barRef}>
      {menus.map((m, i) => (
        <div className="mb" key={m.label}>
          <button type="button" data-top={i} aria-haspopup="menu" aria-expanded={open === i} onClick={() => setOpen(open === i ? null : i)} onMouseEnter={() => open !== null && setOpen(i)}>
            {m.label}
          </button>
          <div className={`menu ${open === i ? 'open' : ''}`} role="menu" aria-label={m.label} data-menu={i} onKeyDown={(e) => onMenuKey(e, i)}>
            {renderItems(m.items, m.label)}
          </div>
        </div>
      ))}
    </nav>
  );
}
