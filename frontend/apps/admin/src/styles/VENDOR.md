# Admin styles

The admin application is a single Tailwind v4 and shadcn design system.

- `globals.css` is the canonical Tailwind entry. It loads Tailwind's three
  explicit layers once, imports the official `tw-animate-css` utilities, and
  owns production-only source discovery.
- `admin-shadcn.css` owns Inter and shadcn's token-to-utility map.
- `admin-theme.css` owns light/dark tokens, operator brand mappings, and the
  global base layer.
- `globals.css` is the sole runtime stylesheet entry and fixes the cascade order.

The four packaged theme files and their Ant Design, Bootstrap, OneUI, Font
Awesome, and Simple Line Icons payloads have been removed. Operator color
choices now select a small shadcn variable palette through
`data-theme-color`; they never inject another stylesheet into the cascade.
