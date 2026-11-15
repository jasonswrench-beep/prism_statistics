import { Component, StrictMode, type ErrorInfo, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles.css';
import { watchDialogs } from './dialogfocus';

/** If something unexpected breaks a screen, say so in plain words instead of leaving a blank page. */
class Fallback extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('The interface stopped:', error, info.componentStack);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <main className="empty-state" role="alert">
        <h1>PrismStatistics hit a problem</h1>
        <p className="muted">{this.state.error.message}</p>
        <button type="button" className="btn primary" onClick={() => window.location.reload()}>
          Reload
        </button>
      </main>
    );
  }
}

const root = document.getElementById('root');
if (!root) throw new Error('The page has no #root element.');
// Focus, Tab and the arrow keys in every dialog (dialogfocus.ts).
watchDialogs(document);
createRoot(root).render(
  <StrictMode>
    <Fallback>
      <App />
    </Fallback>
  </StrictMode>,
);
