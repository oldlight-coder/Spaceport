import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import { applyTheme, cachedAppearance } from "./theme";

// The saved look is painted before the first render; settings may update it once loaded.
applyTheme(cachedAppearance());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
