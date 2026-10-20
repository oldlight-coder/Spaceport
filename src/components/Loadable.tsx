import { ReactNode, useCallback, useEffect, useState } from "react";

export type Load<T> =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; data: T };

/** Runs `load` on mount and returns its state plus a reload function. */
export function useLoad<T>(load: () => Promise<T>) {
  const [state, setState] = useState<Load<T>>({ status: "loading" });
  const reload = useCallback(() => {
    setState({ status: "loading" });
    load()
      .then((data) => setState({ status: "ready", data }))
      .catch((e) => setState({ status: "error", message: String(e) }));
  }, [load]);
  useEffect(reload, [reload]);
  return [state, reload, setState] as const;
}

interface LoadableListProps<T> {
  state: Load<T[]>;
  onRetry: () => void;
  empty: string;
  children: (data: T[]) => ReactNode;
}

/** Shows loading, error with retry, an empty message, or the list. */
export function LoadableList<T>({ state, onRetry, empty, children }: LoadableListProps<T>) {
  switch (state.status) {
    case "loading":
      return <p className="muted">Loading…</p>;
    case "error":
      return (
        <div className="inline-error">
          <p className="error">{state.message}</p>
          <button className="secondary" onClick={onRetry}>
            Retry
          </button>
        </div>
      );
    case "ready":
      return state.data.length === 0 ? <p className="muted">{empty}</p> : <>{children(state.data)}</>;
  }
}
