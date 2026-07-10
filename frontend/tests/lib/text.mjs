// Shared text-normalization helpers for the dual-world parity suite.
//
// The interaction suite runs one run(page) against both the shadcn source build
// and the frozen antd oracle, then compares the normalized results. antd injects
// a rendering-only space between two CJK characters (`取 消` → `取消`); the shadcn
// redesign drops it. Collapsing that space identically on both worlds can only
// make an insignificant difference disappear, never mask a genuine mismatch.

const cjkTextRange = '\\u3040-\\u30ff\\u3400-\\u9fff\\uf900-\\ufaff\\uac00-\\ud7af';
export const cjkInnerSpacePattern = new RegExp(`([${cjkTextRange}]) (?=[${cjkTextRange}])`, 'g');

export function normalizeParityText(value) {
  return String(value ?? '')
    .trim()
    .replace(/\s+/g, ' ')
    .replace(cjkInnerSpacePattern, '$1');
}

export function collapseCjkDeep(value) {
  if (typeof value === 'string') {
    return value.replace(cjkInnerSpacePattern, '$1');
  }
  if (Array.isArray(value)) {
    return value.map(collapseCjkDeep);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, nested]) => [key, collapseCjkDeep(nested)]),
    );
  }
  return value;
}
