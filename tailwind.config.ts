import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "#0B0D13",
        surface: {
          DEFAULT: "#141721",
          panel: "#141721",
          border: "#252B3B",
        },
        primary: {
          DEFAULT: "#6366F1",
          hover: "#4F46E5",
        },
        amber: {
          accent: "#F59E0B",
          glow: "rgba(245, 158, 11, 0.25)",
        },
        slate: {
          muted: "#94A3B8",
          dark: "#1E293B",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "Cascadia Code", "Consolas", "monospace"],
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"],
      },
    },
  },
  plugins: [],
} satisfies Config;
