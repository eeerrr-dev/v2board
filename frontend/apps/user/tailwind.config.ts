import type { Config } from 'tailwindcss';

const config: Config = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        brand: {
          DEFAULT: '#0665d0',
          50: '#e6f0fb',
          100: '#cfe2f7',
          200: '#9ec5ef',
          300: '#6ea8e7',
          400: '#3e8bdf',
          500: '#0665d0',
          600: '#0551a6',
          700: '#043d7d',
          800: '#022853',
          900: '#01142a',
        },
      },
      fontFamily: {
        sans: [
          '"Inter"',
          '"PingFang SC"',
          '"Hiragino Sans"',
          '"Microsoft YaHei"',
          'system-ui',
          'sans-serif',
        ],
      },
      borderRadius: {
        DEFAULT: '0.5rem',
      },
    },
  },
};

export default config;
