import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Controls from "./Controls";
import "./player.css";
import { applyTheme, cachedAppearance } from "../theme";

// The accent follows the main window's choice (shared storage); the controls stay dark over
// the video.
applyTheme(cachedAppearance(), true);
window.addEventListener("storage", () => applyTheme(cachedAppearance(), true));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Controls />
  </StrictMode>,
);
