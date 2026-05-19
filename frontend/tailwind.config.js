/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: '#0f172a',
        surface: '#1e293b',
        primary: '#6366f1',
        secondary: '#94a3b8',
      }
    },
  },
  plugins: [
    require('@vidstack/react/tailwind.cjs')()
  ],
}
