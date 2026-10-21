import { ReactNode, useCallback, useEffect, useState } from "react";
import { useRetryWhenOnline } from "../online";
import { ListSkeleton } from "./Skeleton";

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
  // A load that failed (often for want of a connection) is tried again once it is back.
  useRetryWhenOnline(reload, state.status === "error");
  return [state, reload, setState] as const;
}

interface LoadableListProps<T> {
  state: Load<T[]>;
  onRetry: () => void;
  empty: ReactNode;
  /** Shown while loading; lines of text when not given. */
  skeleton?: ReactNode;
  children: (data: T[]) => ReactNode;
}

/** Shows loading, error with retry, an empty message, or the list. */
export function LoadableList<T>({
  state,
  onRetry,
  empty,
  skeleton,
  children,
}: LoadableListProps<T>) {
  switch (state.status) {
    case "loading":
      return <>{skeleton ?? <ListSkeleton rows={3} />}</>;
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
      if (state.data.length > 0) return <>{children(state.data)}</>;
      return typeof empty === "string" ? <p className="muted">{empty}</p> : <>{empty}</>;
  }
}
