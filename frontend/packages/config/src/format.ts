const BYTES_PER_GB = 1_073_741_824;
const BYTES_PER_MB = 1_048_576;

export function formatBytes(bytes: number | string, fractionDigits = 2): string | number {
  const value = Number.parseInt(String(bytes), 10);
  if (value > BYTES_PER_GB) return `${(value / BYTES_PER_GB).toFixed(fractionDigits)} GB`;
  if (value > BYTES_PER_MB) return `${(value / BYTES_PER_MB).toFixed(fractionDigits)} MB`;
  if (value > 1024) return `${(value / 1024).toFixed(fractionDigits)} KB`;
  if (value < 0) return 0;
  return `${value.toFixed(fractionDigits)} B`;
}

export function formatTrafficGiB(bytes: number, fractionDigits = 2): string {
  return `${(bytes / BYTES_PER_GB).toFixed(fractionDigits)} GB`;
}

export function formatMoney(cents: number, symbol = '¥', fractionDigits = 2): string {
  return `${symbol}${(cents / 100).toFixed(fractionDigits)}`;
}

export function formatDate(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  const d = new Date(timestamp * 1000);
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function formatDateTime(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  const d = new Date(timestamp * 1000);
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function formatDateMinuteSlash(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  const d = new Date(timestamp * 1000);
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function daysUntil(timestamp: number | null | undefined): number | null {
  if (!timestamp) return null;
  const ms = timestamp * 1000 - Date.now();
  return Math.round(ms / 86_400_000);
}

export const BYTE_GB = BYTES_PER_GB;
