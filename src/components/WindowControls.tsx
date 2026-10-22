import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isAndroid } from "../platform";

/** Minimize, maximize or restore, and close, for the title bar Spaceport draws itself. */
export default function WindowControls() {
  if (isAndroid) return null;
  return <WindowButtons />;
}

function WindowButtons() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const win = getCurrentWindow();
    const update = () =>
      win
        .isMaximized()
        .then(setMaximized)
        .catch(() => {});
    update();
    const unlisten = win.onResized(update);
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);
  const win = () => getCurrentWindow();
  return (
    <div className="window-controls">
      <button onClick={() => win().minimize()} aria-label="Minimize" title="Minimize">
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 5h8" />
        </svg>
      </button>
      <button
        onClick={() => win().toggleMaximize()}
        aria-label={maximized ? "Restore" : "Maximize"}
        title={maximized ? "Restore" : "Maximize"}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          {maximized ? (
            <path d="M3 1h6v6M1 3h6v6H1z" />
          ) : (
            <path d="M1 1h8v8H1z" />
          )}
        </svg>
      </button>
      <button className="close" onClick={() => win().close()} aria-label="Close" title="Close">
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 1l8 8M9 1l-8 8" />
        </svg>
      </button>
    </div>
  );
}

/** A bare title bar for screens without the main top bar (start-up, sign-in). */
export function TitleBar() {
  // Android draws its own status bar; there is no window to move or close.
  if (isAndroid) return null;
  return (
    <div className="title-bar" data-tauri-drag-region>
      <span className="brand" data-tauri-drag-region>
        Spaceport
      </span>
      <WindowControls />
    </div>
  );
}
