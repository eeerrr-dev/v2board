/**
 * Authored V2Board — canonical design tokens for the gradual reskin (clean-modern).
 *
 * Single source of truth, framework-neutral. The user app consumes these as a Tailwind v4
 * `@theme` (./tokens.css, locked to this object by the tokens drift test); the admin app will
 * later feed the same values into antd 6's `ConfigProvider` theme. Every token is ADDITIVE — it
 * introduces a new `--color-*` / `--radius-*` / `--shadow-*` name rather than overriding a
 * Tailwind default — so the tokens never disturb un-reskinned (replica) surfaces.
 *
 * The primary hue lives in one place: edit the `--color-primary*` values to rebrand.
 *
 * Dark mode is currently produced by DarkReader (runtime inversion of this light theme); a real
 * token-based dark theme is a separate, cross-cutting decision and is intentionally not defined
 * here.
 */
export const tokens = {
  // Brand ramp (primary) — a confident, slightly deep blue.
  '--color-primary-50': '#eef3ff',
  '--color-primary-100': '#dde7ff',
  '--color-primary-200': '#c2d2ff',
  '--color-primary-500': '#3a63f0',
  '--color-primary-600': '#2457e5',
  '--color-primary-700': '#1d45c0',

  // Semantic colors.
  '--color-primary': '#2457e5',
  '--color-primary-hover': '#1d45c0',
  '--color-primary-foreground': '#ffffff',
  '--color-primary-subtle': '#eef3ff',
  '--color-background': '#f5f7fb',
  '--color-surface': '#ffffff',
  '--color-foreground': '#0f172a',
  '--color-muted': '#f1f5f9',
  '--color-muted-foreground': '#64748b',
  '--color-border': '#e6eaf2',
  '--color-input': '#cbd5e1',
  '--color-ring': '#3a63f0',

  // Radii.
  '--radius-field': '0.625rem',
  '--radius-card': '1.25rem',

  // Elevation.
  '--shadow-card': '0 12px 40px -12px rgba(15, 23, 42, 0.25)',
} as const;

export type DesignTokenName = keyof typeof tokens;
