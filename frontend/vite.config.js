import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { folderPickerPlugin } from "./vite-folder-picker-plugin.js";

export default defineConfig({
  plugins: [react(), folderPickerPlugin()],
  server: {
    port: 8000,
    strictPort: true,
  },
});
