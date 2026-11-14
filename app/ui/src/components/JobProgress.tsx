// Progress and Cancel for a long analysis. When a command has been running for a moment and the engine reports a job,
// a small panel shows how far it has come, with a Cancel button. A cancelled analysis adds nothing to the output.

import { useEffect, useState } from 'react';
import { call, watchPending } from '../bridge';

type Job = { id: number; label: string; done: number; total: number; seconds: number; cancelling: boolean };

export function JobProgress() {
  const [busy, setBusy] = useState(false);
  const [job, setJob] = useState<Job | null>(null);

  useEffect(() => {
    let timer: number | undefined;
    const stop = watchPending((n) => {
      window.clearTimeout(timer);
      if (n > 0) timer = window.setTimeout(() => setBusy(true), 700);
      else {
        setBusy(false);
        setJob(null);
      }
    });
    return () => {
      stop();
      window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    if (!busy) return;
    let live = true;
    const poll = async () => {
      try {
        const jobs = await call<Job[]>('job_status');
        if (live) setJob(jobs[0] ?? null);
      } catch {
        // The panel is a convenience; a failed poll just waits for the next one.
      }
    };
    poll();
    const t = window.setInterval(poll, 400);
    return () => {
      live = false;
      window.clearInterval(t);
    };
  }, [busy]);

  if (!busy || !job) return null;
  const pct = job.total > 0 ? Math.min(100, (100 * job.done) / job.total) : null;
  return (
    <div className="jobprogress" role="status" aria-live="polite">
      <div className="jobprogress-text">
        <strong>{job.cancelling ? 'Cancelling…' : 'Running the analysis…'}</strong>
        <span className="fieldhint">
          {pct !== null ? `${job.done.toLocaleString()} of ${job.total.toLocaleString()} · ` : ''}
          {Math.round(job.seconds)} s
        </span>
      </div>
      <div className={`jobbar${pct === null ? ' indeterminate' : ''}`}>
        <div style={pct === null ? undefined : { width: `${pct}%` }} />
      </div>
      <button type="button" className="btn" disabled={job.cancelling} onClick={() => call('cancel_job', { id: job.id }).catch(() => undefined)}>
        Cancel
      </button>
    </div>
  );
}
