// Explain this result, under each result: what the analysis asks, what each of its numbers means here, the checks that
// need attention, and what the result can't show (explain.ts). Collapsed until asked for.

import { useContext, useMemo, useState } from 'react';
import { dialogFor } from '../analyses';
import { ciSentence, explain } from '../explain';
import { helpFor } from '../help';
import type { OutputItem } from '../types';
import { HelpContext } from './HelpDialog';
import { Icon } from './Icon';

/** The first paragraph of a help page, after its title: what the analysis does. */
function summary(text: string): string | null {
  const para = text
    .replace(/\r/g, '')
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .find((p) => p && !p.startsWith('#'));
  return para ? para.replace(/\*\*([^*]+)\*\*/g, '$1').replace(/\[([^\]]+)\]\([^)]+\)/g, '$1') : null;
}

export function ExplainBlock({ item }: { item: OutputItem }) {
  const [open, setOpen] = useState(false);
  const { show } = useContext(HelpContext);
  const page = helpFor(item.command ? dialogFor(item.command, item.args ?? {}) : null);
  const e = useMemo(() => (open ? explain(item.bundle) : null), [open, item.bundle]);
  if (item.bundle.status.state !== 'ok' || item.bundle.method_id === 'power') return null;
  const asks = page ? summary(page.text) : null;
  const id = `explain-${item.id}`;
  return (
    <div className="explain">
      <button type="button" className="btn ghost" aria-expanded={open} aria-controls={id} onClick={() => setOpen((o) => !o)}>
        <Icon name="info" />
        {open ? 'Hide the explanation' : 'Explain this result'}
      </button>
      {open && e && (
        <div className="explainbody" id={id}>
          {item.stale && <p className="warnline">This result is out of date: the data changed after it was run, so read it after running it again.</p>}
          {asks && (
            <>
              <h4>What this analysis asks</h4>
              <p>{asks}</p>
            </>
          )}
          {e.readings.length > 0 && (
            <>
              <h4>Reading the numbers</h4>
              <dl>
                {e.readings.map((r) => (
                  <div key={r.term.id}>
                    <dt>
                      <button type="button" className="linkbtn" onClick={() => show('glossary')} title="Open the glossary">
                        {r.term.term}
                      </button>
                      {r.label && <span className="muted"> · {r.label}</span>}
                    </dt>
                    <dd>
                      {r.sentence}
                      {ciSentence(r) && <> {ciSentence(r)}</>}
                    </dd>
                  </div>
                ))}
              </dl>
            </>
          )}
          <h4>Checks</h4>
          {e.checks.length ? (
            <ul>
              {e.checks.map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          ) : (
            <p>None of the checks that were made raised a concern.</p>
          )}
          {e.unchecked.length > 0 && (
            <>
              <p>Not checked for you:</p>
              <ul>
                {e.unchecked.map((c) => (
                  <li key={c}>{c}</li>
                ))}
              </ul>
            </>
          )}
          <p>Some assumptions, such as independent observations, can’t be checked from the data at all: they depend on how the data were collected.</p>
          {e.cautions.length > 0 && (
            <>
              <h4>Keep in mind</h4>
              <ul>
                {e.cautions.map((c) => (
                  <li key={c}>{c}</li>
                ))}
              </ul>
            </>
          )}
          <p className="fieldhint">
            Every number here comes from the result above.{' '}
            {page && (
              <button type="button" className="linkbtn" onClick={() => show(page.id)}>
                More about {page.title}
              </button>
            )}
          </p>
        </div>
      )}
    </div>
  );
}
