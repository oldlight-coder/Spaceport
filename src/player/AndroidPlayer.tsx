import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import Controls from "./Controls";
import playerCss from "./player.css?inline";
import { onPlayerEvent } from "../api/player";
import { cachedAppearance, themeVariables } from "../theme";

/**
 * On Android the video plays in a native view under the page, so the page draws the player
 * controls itself while a video is open (on Windows they have a window of their own).
 */
export default function AndroidPlayer() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const stop = onPlayerEvent((event) => {
      if (event.kind === "opened") setOpen(true);
      if (event.kind === "closed" || event.kind === "failed") setOpen(false);
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  }, []);
  return open ? <ControlsHost /> : null;
}

/**
 * The controls in a shadow root with their own React root, so the player's styles and the
 * page's leave each other alone, as they do in separate windows.
 */
function ControlsHost() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const shadow = element.shadowRoot ?? element.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    // The rules player.css gives the controls window's page belong to the host here.
    style.textContent = playerCss.replace(/:root\b/g, ":host");
    const container = document.createElement("div");
    container.id = "root";
    shadow.replaceChildren(style, container);
    // Dark colours over the video, whatever the page uses.
    const { vars } = themeVariables(cachedAppearance(), true);
    for (const [name, value] of Object.entries(vars)) element.style.setProperty(name, value);
    const root = createRoot(container);
    root.render(<Controls />);
    // Unmounting during the page's own render is not allowed, so it waits a tick.
    return () => {
      window.setTimeout(() => root.unmount());
    };
  }, []);
  return <div ref={host} className="android-player" />;
}
