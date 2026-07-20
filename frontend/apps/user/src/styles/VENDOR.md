# User styles

The user application is a single Tailwind v4 and shadcn design system.

- `globals.css` loads Tailwind's three explicit layers once, imports the official
  `tw-animate-css` utilities, and owns production-only source discovery.
- `@v2board/ui/styles/shadcn.css` owns the shared Inter font and shadcn
  token-to-utility map.
- `@v2board/ui/styles/theme.css` owns the shared light/dark token values,
  operator brand mappings, and global base layer.
- `user-custom-html.css` is token-native prose scoped to `.custom-html-style`
  for backend-authored knowledge, notice, and plan markup that cannot
  carry Tailwind utility classes.
- `globals.css` is the sole runtime stylesheet entry and imports the theme,
  utilities, and scoped rich-content rules in cascade order.

Bootstrap, OneUI, Ant Design, global legacy element typography, legacy theme
stylesheets, and icon fonts are not runtime dependencies. Shared primitives
belong to `@v2board/ui`; user-only compositions remain local and use Lucide icons.
