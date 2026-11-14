// The help pages, bundled with the interface so they work offline: the user guide's chapters (help/guide, in the
// order of their file names) and its Glossary (glossary.ts), the worked examples (help/examples), and one page per dialog
// (help/topics/<dialog id>.md).

import { glossaryPage } from '../glossary';

export type HelpPage = { id: string; title: string; text: string; guide: boolean; example?: boolean };

const guide = import.meta.glob('./guide/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const examples = import.meta.glob('./examples/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const topics = import.meta.glob('./topics/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const page = (path: string, text: string, isGuide: boolean): HelpPage => {
  const file = path.split('/').pop()!.replace(/\.md$/, '');
  return { id: isGuide ? file.replace(/^\d+-/, '') : file, title: /^# (.+)$/m.exec(text)?.[1].trim() ?? file, text, guide: isGuide };
};

export const GUIDE: HelpPage[] = Object.entries(guide)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([p, t]) => page(p, t, true))
  .concat([{ id: 'glossary', title: 'Glossary', text: glossaryPage(), guide: true }]);

export const EXAMPLES: HelpPage[] = Object.entries(examples)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([p, t]) => ({ ...page(p, t, true), guide: false, example: true }));

export const TOPICS: HelpPage[] = Object.entries(topics)
  .map(([p, t]) => page(p, t, false))
  .sort((a, b) => a.title.localeCompare(b.title));

export const ALL: HelpPage[] = [...GUIDE, ...EXAMPLES, ...TOPICS];

/** The help page for a dialog, if there is one. Some dialogs open under another id. */
export function helpFor(dialog: string | null): HelpPage | undefined {
  if (!dialog) return undefined;
  const id = dialog === 'design' ? 'doe_design' : dialog;
  return ALL.find((p) => p.id === id);
}
