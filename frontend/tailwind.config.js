/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,jsx}"],
  theme: {
    extend: {
      colors: {
        accent: "var(--accent)",
        "bg-primary": "var(--bg-primary)",
        "bg-panel": "var(--bg-panel)",
        "text-primary": "var(--text-primary)",
        "text-dim": "var(--text-dim)",
        danger: "var(--danger)",
        warning: "var(--warning)",
      },
      fontFamily: {
        display: ["Rajdhani", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
      boxShadow: {
        glow: "0 0 12px color-mix(in srgb, var(--accent) 45%, transparent)",
      },
    },
  },
  plugins: [],
};
