/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Outfit", "ui-sans-serif", "system-ui", "sans-serif"],
      },
      colors: {
        surface: {
          DEFAULT: "#fafafa",
          dark: "#18181b",
        },
        paper: {
          DEFAULT: "#ffffff",
          dark: "#27272a",
        },
        ink: {
          DEFAULT: "#18181b",
          mute: "#71717a",
          dark: "#fafafa",
          "dark-mute": "#a1a1aa",
        },
        accent: {
          DEFAULT: "#3b82f6",
          hover: "#2563eb",
          light: "#eff6ff",
          "light-dark": "#1e3a8a",
        },
        border: {
          DEFAULT: "#e5e7eb",
          dark: "#3f3f46",
        },
      },
      keyframes: {
        "slide-in-right": {
          from: { transform: "translateX(100%)" },
          to: { transform: "translateX(0)" },
        },
      },
      animation: {
        "slide-in-right": "slide-in-right 0.2s ease-out",
      },
    },
  },
  plugins: [],
};
