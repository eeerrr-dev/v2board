export function getLegacyCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    const [key, raw] = item.split('=');
    return key === name && raw !== undefined ? decodeURIComponent(raw) : value;
  }, '');
}

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
