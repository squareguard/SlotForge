import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";
import SlotForgeApp from "./SlotForgeApp.jsx";
import { ErrorBoundary } from "./ErrorBoundary.jsx";

createRoot(document.getElementById("root")).render(
  <StrictMode>
    <ErrorBoundary>
      <SlotForgeApp />
    </ErrorBoundary>
  </StrictMode>
);
