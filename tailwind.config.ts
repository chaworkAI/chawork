import type { Config } from "tailwindcss"

export default {
  content: ["./frontend/**/*.{ts,tsx}", "./frontend/index.html"],
  theme: {
    extend: {
      screens: {
        shell: "900px",
        "shell-lg": "1200px",
      },
      colors: {
        "shell-bg": "var(--shell-bg)",
        "shell-surface": "var(--shell-surface)",
        surface: "var(--shell-surface)",
        panel: "var(--panel)",
        "panel-soft": "var(--panel-soft)",
        "panel-raised": "var(--panel-raised)",
        "panel-strong": "var(--panel-strong)",
        ink: "var(--text-main)",
        line: "var(--line)",
        "line-soft": "var(--line-soft)",
        "line-strong": "var(--line-strong)",
        accent: "var(--accent-token)",
        "accent-dark": "var(--accent-dark)",
        success: "var(--success)",
        warning: "var(--warning)",
        danger: "var(--danger)",
      },
      borderRadius: {
        panel: "18px",
        card: "8px",
        btn: "12px",
      },
      fontFamily: {
        serif: [
          "ui-serif",
          "Iowan Old Style",
          "Palatino Linotype",
          "Palatino",
          "Georgia",
          "serif",
        ],
      },
      boxShadow: {
        panel: "0 18px 48px rgba(17, 24, 39, 0.08)",
        topbar: "0 10px 28px rgba(17, 24, 39, 0.06)",
      },
    },
  },
  plugins: [],
} satisfies Config
