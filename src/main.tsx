import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import "./styles.css";
import { applyTheme, cachedAppearance } from "./theme";
import { isAndroid } from "./platform";

// Lets the stylesheet adapt to the platform (Android has no window buttons, uses touch).
document.documentElement.dataset.platform = isAndroid ? "android" : "windows";

// The saved look is painted before the first render; settings may update it once loaded.
applyTheme(cachedAppearance());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
