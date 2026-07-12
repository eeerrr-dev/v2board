import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Regression guard for Tailwind v4 dialog centering. The `-translate-*-1/2`
// utilities compile to the independent `translate:` property. The official
// tw-animate-css zoom keyframe uses `transform`, so the two compose safely as
// long as the dialog does not also request a 50% slide animation.
const here = dirname(fileURLToPath(import.meta.url));
const globalsCss = readFileSync(resolve(here, 'globals.css'), 'utf8');
const dialogTsx = readFileSync(resolve(here, '../components/ui/dialog.tsx'), 'utf8');
const alertTsx = readFileSync(resolve(here, '../components/ui/alert-dialog.tsx'), 'utf8');
const surfaceTs = readFileSync(resolve(here, '../components/ui/dialog-surface.ts'), 'utf8');
const dropdownTsx = readFileSync(resolve(here, '../components/ui/dropdown-menu.tsx'), 'utf8');
const pageTsx = readFileSync(resolve(here, '../components/ui/page.tsx'), 'utf8');
const selectTsx = readFileSync(resolve(here, '../components/ui/select.tsx'), 'utf8');
const sheetTsx = readFileSync(resolve(here, '../components/ui/sheet.tsx'), 'utf8');
const tooltipTsx = readFileSync(resolve(here, '../components/ui/tooltip.tsx'), 'utf8');

describe('dialog centering composes with official shadcn motion', () => {
  it('loads tw-animate-css once and does not keep an app-owned keyframe fallback', () => {
    expect(globalsCss.match(/@import 'tw-animate-css'/g)).toHaveLength(1);
    expect(globalsCss).not.toContain('user-shadcn-motion.css');
    expect(surfaceTs).toContain('data-[state=open]:zoom-in-95');
    expect(surfaceTs).toContain('motion-reduce:animate-none!');
    expect(surfaceTs).not.toMatch(/\bslide-(?:in|out)-(?:from|to)-/);
  });

  it('centers via the -translate-*-1/2 utilities on the shared dialog surface', () => {
    // The centering geometry lives once in dialog-surface.ts; both Radix roots
    // consume it through dialogContentClassName so they cannot drift apart.
    expect(surfaceTs).toContain('-translate-x-1/2');
    expect(surfaceTs).toContain('-translate-y-1/2');
    for (const source of [dialogTsx, alertTsx]) {
      expect(source).toContain('dialogContentClassName');
    }
  });

  it('keeps long dialogs inside the dynamic viewport with internal scrolling', () => {
    expect(surfaceTs).toContain('max-h-[calc(100svh-2rem)]');
    expect(surfaceTs).toContain('overflow-y-auto');
    expect(surfaceTs).toContain('overscroll-contain');
  });

  it('the @source scope scans .ts so dialog-surface.ts utilities are generated', () => {
    // dialog-surface.ts is a plain `.ts` module. `@tailwindcss/vite` only
    // generates a utility when it sees the candidate in a scanned source, so an
    // `@source '**/*.tsx'` glob (tsx only) prunes `left-1/2`/`-translate-x-1/2`
    // — referenced by no `.tsx` — and every modal loses horizontal centering,
    // rendering pinned to the left edge. The components glob must cover `.ts`.
    const componentsSource = globalsCss
      .split('\n')
      .find((line) => line.includes('@source') && line.includes('../**/*'));
    expect(componentsSource, '@source production glob not found in globals.css').toBeDefined();
    // Match `{ts,tsx}` or a bare `*.ts` glob, but not `*.tsx` alone.
    expect(componentsSource).toMatch(/\{ts,tsx\}|\*\.ts['"]/);
  });

  it('uses Radix public origin variables and covers every tooltip open state', () => {
    expect(dropdownTsx).toContain('origin-(--radix-dropdown-menu-content-transform-origin)');
    expect(selectTsx).toContain('origin-(--radix-select-content-transform-origin)');
    expect(tooltipTsx).toContain('origin-(--radix-tooltip-content-transform-origin)');

    // Tooltip.Content can be delayed-open or instant-open. A base enter class
    // covers both states; closed remains the only exit override.
    expect(tooltipTsx).toContain('transform-origin) animate-in');
    expect(tooltipTsx).toContain('data-[state=closed]:animate-out');
    expect(tooltipTsx).not.toContain('data-[state=delayed-open]:animate-in');

    for (const source of [dropdownTsx, selectTsx, tooltipTsx]) {
      expect(source).not.toContain('--radix-popper-transform-origin');
      expect(source).toContain('data-[side=bottom]:slide-in-from-top-2');
      expect(source).toContain('data-[side=left]:slide-in-from-right-2');
      expect(source).toContain('data-[side=right]:slide-in-from-left-2');
      expect(source).toContain('data-[side=top]:slide-in-from-bottom-2');
    }
  });

  it('keeps every migrated surface motion-safe without a custom CSS fallback', () => {
    for (const source of [surfaceTs, dropdownTsx, pageTsx, selectTsx, sheetTsx, tooltipTsx]) {
      // Radix state selectors are more specific than an ordinary media
      // variant. Tailwind v4's trailing important modifier guarantees reduced
      // motion wins for both open and closed keyframes.
      expect(source).toContain('motion-reduce:animate-none!');
      expect(source).not.toContain('v2board-radix-');
    }

    expect(sheetTsx).toContain('data-[state=open]:slide-in-from-top');
    expect(sheetTsx).toContain('data-[state=closed]:slide-out-to-top');
    expect(sheetTsx).toContain('data-[state=open]:slide-in-from-bottom');
    expect(sheetTsx).toContain('data-[state=closed]:slide-out-to-bottom');
    expect(sheetTsx).toContain('data-[state=open]:slide-in-from-left');
    expect(sheetTsx).toContain('data-[state=closed]:slide-out-to-left');
    expect(sheetTsx).toContain('data-[state=open]:slide-in-from-right');
    expect(sheetTsx).toContain('data-[state=closed]:slide-out-to-right');
  });
});
