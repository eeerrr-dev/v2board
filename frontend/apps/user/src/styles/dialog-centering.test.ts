import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Regression guard for the Tailwind v4 transform-stacking bug.
//
// `-translate-x-1/2 -translate-y-1/2` compiles, under Tailwind v4, to the
// INDEPENDENT `translate:` CSS property -- not `transform:`. The browser applies
// `translate:` AND `transform:` together, so if a centering @keyframes also sets
// `transform: translate(-50%, ...)`, the two stack to -100%/-100% and the modal
// renders in the upper-left instead of centered. The defect is invisible to the
// jsdom unit suite (no layout engine), to pixel-retired visual parity, and to
// interaction parity (never compares pixels), so this source assertion pins it.
const here = dirname(fileURLToPath(import.meta.url));
const motionCss = readFileSync(resolve(here, 'user-shadcn-motion.css'), 'utf8');
const dialogTsx = readFileSync(resolve(here, '../components/ui/shadcn-dialog.tsx'), 'utf8');
const alertTsx = readFileSync(resolve(here, '../components/ui/alert-dialog.tsx'), 'utf8');
const surfaceTs = readFileSync(resolve(here, '../components/ui/dialog-surface.ts'), 'utf8');

function keyframeBody(css: string, name: string): string {
  const match = css.match(new RegExp(`@keyframes\\s+${name}\\s*\\{([\\s\\S]*?)^\\}`, 'm'));
  if (!match) throw new Error(`@keyframes ${name} not found in user-shadcn-motion.css`);
  return match[1]!;
}

describe('dialog centering is owned by the translate: utility, not the keyframe', () => {
  it('the dialog open/close keyframes never re-apply translate(-50%, ...)', () => {
    for (const name of ['v2board-dialog-in', 'v2board-dialog-out']) {
      // A bare `translate(-50%` inside the keyframe's `transform:` would stack on
      // top of the `-translate-*-1/2` utility's `translate:` property.
      expect(keyframeBody(motionCss, name)).not.toMatch(/translate\(\s*-?50%/);
    }
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

  it('the @source scope scans .ts so dialog-surface.ts utilities are generated', () => {
    // dialog-surface.ts is a plain `.ts` module. `@tailwindcss/vite` only
    // generates a utility when it sees the candidate in a scanned source, so an
    // `@source '**/*.tsx'` glob (tsx only) prunes `left-1/2`/`-translate-x-1/2`
    // — referenced by no `.tsx` — and every modal loses horizontal centering,
    // rendering pinned to the left edge. The components glob must cover `.ts`.
    const shadcnCss = readFileSync(resolve(here, 'user-shadcn.css'), 'utf8');
    const componentsSource = shadcnCss
      .split('\n')
      .find((line) => line.includes('@source') && line.includes('components/'));
    expect(
      componentsSource,
      '@source glob for components/ not found in user-shadcn.css',
    ).toBeDefined();
    // Match `{ts,tsx}` or a bare `*.ts` glob, but not `*.tsx` alone.
    expect(componentsSource).toMatch(/\{ts,tsx\}|\*\.ts['"]/);
  });
});
