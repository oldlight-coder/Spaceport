import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Controls from "./Controls";
import "./player.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Controls />
  </StrictMode>,
);
