import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";
import SlotForgeApp from "./SlotForgeApp.jsx";

createRoot(document.getElementById("root")).render(
  <StrictMode>
    <SlotForgeApp />
  </StrictMode>
);
