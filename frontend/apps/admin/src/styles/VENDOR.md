# Admin styles

The admin application is a single Tailwind v4 and shadcn design system.

- `globals.css` is the canonical Tailwind entry. It loads Tailwind's three
  explicit layers once, imports the official `tw-animate-css` utilities, and
  owns production-only source discovery.
- `@v2board/ui/styles/shadcn.css` owns the shared Inter font and shadcn
  token-to-utility map.
- `@v2board/ui/styles/theme.css` owns shared light/dark tokens, operator brand
  mappings, and the global base layer.
- `globals.css` is the sole runtime stylesheet entry and fixes the cascade order.

The four packaged theme files and their Ant Design, Bootstrap, OneUI, Font
Awesome, and Simple Line Icons payloads have been removed. Operator color
choices now select a small shadcn variable palette through
`data-theme-color`; they never inject another stylesheet into the cascade.
