module.exports = {
  plugins: {
    // Tailwind v4's PostCSS plugin routes generated utilities through Lightning
    // CSS, which applies vendor prefixes itself — a standalone autoprefixer pass
    // is redundant (no authored CSS here needs prefixing beyond it).
    '@tailwindcss/postcss': {},
  },
};
