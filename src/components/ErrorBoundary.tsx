import { Component, ReactNode } from "react";

interface State {
  error: string | null;
}

/** Shows what went wrong, with a way back, instead of a blank window after a render error. */
export default class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: unknown): State {
    return { error: error instanceof Error ? error.message : String(error) };
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return (
      <main className="center">
        <div className="card">
          <h2>Something went wrong</h2>
          <p className="error">{this.state.error}</p>
          <button onClick={() => window.location.reload()}>Reload Spaceport</button>
        </div>
      </main>
    );
  }
}
