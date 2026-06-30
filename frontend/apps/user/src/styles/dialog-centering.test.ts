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

  it('the dialog content elements still center via the -translate-*-1/2 utilities', () => {
    for (const source of [dialogTsx, alertTsx]) {
      expect(source).toContain('-translate-x-1/2');
      expect(source).toContain('-translate-y-1/2');
    }
  });
});
