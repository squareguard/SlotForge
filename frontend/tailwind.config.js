/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,jsx}"],
  theme: {
    borderRadius: {
      none: "0",
      sm: "4px",
      DEFAULT: "6px",
      md: "6px",
      lg: "8px",
      full: "9999px",
    },
    extend: {
      colors: {
        accent: "var(--accent)",
        "bg-primary": "var(--bg-primary)",
        "bg-panel": "var(--bg-panel)",
        "text-primary": "var(--text-primary)",
        "text-dim": "var(--text-dim)",
        danger: "var(--danger)",
        warning: "var(--warning)",
        success: "var(--success)",
      },
      fontFamily: {
        display: ["Rajdhani", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
      fontSize: {
        xs: ["0.75rem", { lineHeight: "1.25rem" }],
        sm: ["0.8125rem", { lineHeight: "1.25rem" }],
        base: ["1rem", { lineHeight: "1.5rem" }],
        lg: ["1.125rem", { lineHeight: "1.5rem" }],
        xl: ["1.25rem", { lineHeight: "1.5rem" }],
        "2xl": ["1.5rem", { lineHeight: "1.75rem" }],
      },
      boxShadow: {
        elevation: "0 4px 12px rgba(0, 0, 0, 0.35)",
      },
    },
  },
  plugins: [],
};
