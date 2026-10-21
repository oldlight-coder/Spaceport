import { useEffect, useState } from "react";

/** Whether Windows reports a network connection; updates as it comes and goes. */
export function useOnline() {
  const [online, setOnline] = useState(navigator.onLine);
  useEffect(() => {
    const update = () => setOnline(navigator.onLine);
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, []);
  return online;
}

/** Runs `retry` when the connection comes back. */
export function useRetryWhenOnline(retry: () => void, active: boolean) {
  useEffect(() => {
    if (!active) return;
    window.addEventListener("online", retry);
    return () => window.removeEventListener("online", retry);
  }, [retry, active]);
}
