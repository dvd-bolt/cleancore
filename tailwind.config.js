/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        bg: '#171819',
        shell: '#212325',
        surface: {
          DEFAULT: '#2a2c2f',
          2: '#36393d',
        },
        line: '#484c52',
        txt: {
          DEFAULT: '#f1f2f4',
          muted: '#c1c4ca',
          dim: '#a6aab3',
        },
        accent: {
          DEFAULT: '#e0e3e9',
          ink: '#25282e',
          wash: '#35393f',
        },
      },
      fontFamily: {
        sans: ['"Trebuchet MS"', 'system-ui', 'sans-serif'],
        heading: ['Bahnschrift', '"Trebuchet MS"', 'sans-serif'],
        mono: ['"Cascadia Mono"', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
}
