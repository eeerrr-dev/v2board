import { readFileSync, readdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Enumerate the styles directory instead of keeping a hand-maintained file
// list: a hand list let a newly added, properly-imported stylesheet
// reintroduce banned selectors without ever being scanned by the
// globals.test.ts guards (the exact drift mode the `@source` comment in
// user-shadcn.css documents). Together with styles-reachability.test.ts —
// which guards the opposite direction (files on disk that the runtime never
// imports) — every stylesheet on disk is both scanned and reachable.
const stylesDir = resolve(dirname(fileURLToPath(import.meta.url)), '../styles');

export function readUserStyles() {
  return readdirSync(stylesDir)
    .filter((name) => name.endsWith('.css'))
    .sort()
    .map((name) => readFileSync(resolve(stylesDir, name), 'utf8'))
    .join('');
}
