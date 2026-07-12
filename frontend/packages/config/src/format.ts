import dayjs from 'dayjs';

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

// Symbol-less cents -> decimal string. The caller composes its own currency
// symbol/spacing/suffix around this (invite commission cells, profile balance),
// which is why it stays separate from formatMoney's symbol-prefixed shape.
export function formatCentsPlain(cents: number | string): string {
  return (parseInt(String(cents)) / 100).toFixed(2);
}

export function formatDate(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  return dayjs(timestamp * 1000).format('YYYY-MM-DD');
}

export function formatDateTime(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  return dayjs(timestamp * 1000).format('YYYY-MM-DD HH:mm:ss');
}

export function formatBackendDate(timestamp: number | string | null | undefined): string {
  const d = dayjs(Number(timestamp) * 1000);
  if (!d.isValid()) return 'Invalid date';
  return d.format('YYYY-MM-DD');
}

export function formatBackendDateTime(timestamp: number | string | null | undefined): string {
  const d = dayjs(Number(timestamp) * 1000);
  if (!d.isValid()) return 'Invalid date';
  return d.format('YYYY-MM-DD HH:mm:ss');
}

export function formatDateMinuteSlash(timestamp: number | null | undefined): string {
  if (!timestamp) return '-';
  return dayjs(timestamp * 1000).format('YYYY/MM/DD HH:mm');
}

export function formatBackendDateSlash(timestamp: number | string | null | undefined): string {
  const d = dayjs(Number(timestamp) * 1000);
  if (!d.isValid()) return 'Invalid date';
  return d.format('YYYY/MM/DD');
}

export function formatBackendDateMinuteSlash(
  timestamp: number | string | null | undefined,
): string {
  const d = dayjs(Number(timestamp) * 1000);
  if (!d.isValid()) return 'Invalid date';
  return d.format('YYYY/MM/DD HH:mm');
}

export function daysUntil(timestamp: number | null | undefined): number | null {
  if (!timestamp) return null;
  return Number(((timestamp - Math.floor(Date.now() / 1000)) / 86_400).toFixed(0));
}

export const BYTE_GB = BYTES_PER_GB;
