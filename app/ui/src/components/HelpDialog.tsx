// Help › PrismStatistics Help (F1): the user guide and a page for every dialog, with search. The ? button in a dialog's
// title bar, or F1 while it is open, opens that dialog's page.

import { createContext, useContext, useMemo, useState } from 'react';
import { ALL, EXAMPLES, GUIDE, TOPICS, helpFor, type HelpPage } from '../help';
import { Markdown } from '../help/markdown';
import { Icon } from './Icon';

/** The dialog that is open, so its ? button knows which page to show; and how to show a page. */
export const HelpContext = createContext<{ dialog: string | null; show: (id: string) => void }>({ dialog: null, show: () => undefined });

export function HelpButton() {
  const { dialog, show } = useContext(HelpContext);
  const page = helpFor(dialog);
  if (!page) return null;
  return (
    <button type="button" className="iconbtn helpbtn" aria-label={`Help: ${page.title}`} title="Help (F1)" onClick={() => show(page.id)}>
      ?
    </button>
  );
}

export function HelpDialog({ start, onClose, onExample }: { start: string | null; onClose: () => void; onExample?: (id: string) => void }) {
  const [id, setId] = useState(start && ALL.some((p) => p.id === start) ? start : GUIDE[0]?.id ?? '');
  const [query, setQuery] = useState('');
  const page = ALL.find((p) => p.id === id);
  const q = query.trim().toLowerCase();
  const match = (p: HelpPage) => !q || p.title.toLowerCase().includes(q) || p.text.toLowerCase().includes(q);
  const [guide, examples, topics] = useMemo(() => [GUIDE.filter(match), EXAMPLES.filter(match), TOPICS.filter(match)], [q]); // eslint-disable-line react-hooks/exhaustive-deps
  // A link inside a page: another page, or an example dataset to open.
  const follow = (target: string) => (target.startsWith('example:') ? onExample?.(target.slice(8)) : setId(target));
  const item = (p: HelpPage) => (
    <li key={p.id}>
      <button type="button" className={p.id === id ? 'on' : ''} aria-current={p.id === id ? 'page' : undefined} onClick={() => setId(p.id)}>
        {p.title}
      </button>
    </li>
  );
  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog helpdialog" role="dialog" aria-modal="true" aria-labelledby="helpTitle">
        <header>
          <h2 id="helpTitle">PrismStatistics Help</h2>
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose} autoFocus>
            <Icon name="close" />
          </button>
        </header>
        <div className="helpbody">
          <nav className="helpnav" aria-label="Help pages">
            <input type="search" placeholder="Search help" aria-label="Search help" value={query} onChange={(e) => setQuery(e.target.value)} />
            {guide.length > 0 && (
              <>
                <h4>User guide</h4>
                <ul>{guide.map(item)}</ul>
              </>
            )}
            {examples.length > 0 && (
              <>
                <h4>Worked examples</h4>
                <ul>{examples.map(item)}</ul>
              </>
            )}
            {topics.length > 0 && (
              <>
                <h4>Analyses and tools</h4>
                <ul>{topics.map(item)}</ul>
              </>
            )}
            {guide.length + examples.length + topics.length === 0 && <p className="fieldhint">No page mentions “{query}”.</p>}
          </nav>
          <article className="helppage" aria-live="polite">
            {page ? (
              <>
                <h2>{page.title}</h2>
                <Markdown text={page.text} open={follow} />
              </>
            ) : (
              <p>Choose a page.</p>
            )}
          </article>
        </div>
      </div>
    </div>
  );
}
