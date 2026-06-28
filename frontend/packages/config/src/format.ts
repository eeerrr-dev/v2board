const BYTES_PER_GB = 1_073_741_824;
const BYTES_PER_MB = 1_048_576;

export function formatBytes(bytes: number | string = 0, fractionDigits = 2): string | number {
  const value = parseInt(String(bytes));
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

export function legacyEpochDate(timestamp: number | string | null | undefined): Date | null {
  const date = new Date(Number(timestamp) * 1000);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function padLegacyDatePart(value: number): string {
  return `${value}`.padStart(2, '0');
}

export function formatLegacyDateTime(timestamp: number | string | null | undefined): string {
  const d = legacyEpochDate(timestamp);
  if (!d) return 'Invalid date';
  const pad = padLegacyDatePart;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function formatDateMinuteSlash(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  const d = new Date(timestamp * 1000);
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function formatLegacyDateMinuteSlash(timestamp: number | string | null | undefined): string {
  const d = legacyEpochDate(timestamp);
  if (!d) return 'Invalid date';
  const pad = padLegacyDatePart;
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function daysUntil(timestamp: number | null | undefined): number | null {
  if (!timestamp) return null;
  return Number(((timestamp - Math.floor(Date.now() / 1000)) / 86_400).toFixed(0));
}

export const BYTE_GB = BYTES_PER_GB;
