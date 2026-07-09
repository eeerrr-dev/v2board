// The reader lives in @v2board/i18n, which parses the same legacy `i18n` cookie
// during boot; re-export the shared implementation instead of keeping a second
// copy of the parsing semantics.
export { getLegacyCookie } from '@v2board/i18n';

export function setLegacyCookie(
  name: string,
  value: string | number,
  minutes = 525600,
  path = '/',
  domain?: string,
): void {
  const expires = new Date(Date.now() + minutes * 60_000).toUTCString();
  document.cookie =
    `${name}=${encodeURIComponent(value)};expires=${expires};path=${path}` +
    (domain ? `;domain=${domain}` : '');
}
