import {
  formatLegacyDateMinuteSlash as formatBaseLegacyDateMinuteSlash,
  formatLegacyDateTime as formatBaseLegacyDateTime,
} from '@v2board/config/format';

function legacyDate(timestamp: number | string | null | undefined): Date | null {
  const date = new Date(Number(timestamp) * 1000);
  return Number.isNaN(date.getTime()) ? null : date;
}

function padLegacyDatePart(value: number): string {
  return `${value}`.padStart(2, '0');
}

export function formatUserLegacyDate(
  timestamp: number | string | null | undefined,
): string {
  const date = legacyDate(timestamp);
  if (!date) return 'Invalid date';
  return `${date.getFullYear()}-${padLegacyDatePart(date.getMonth() + 1)}-${padLegacyDatePart(date.getDate())}`;
}

export function formatUserLegacyDateSlash(
  timestamp: number | string | null | undefined,
): string {
  const date = legacyDate(timestamp);
  if (!date) return 'Invalid date';
  return `${date.getFullYear()}/${padLegacyDatePart(date.getMonth() + 1)}/${padLegacyDatePart(date.getDate())}`;
}

export function formatUserLegacyDateTime(
  timestamp: number | string | null | undefined,
): string {
  return formatBaseLegacyDateTime(timestamp);
}

export function formatUserLegacyDateMinuteSlash(
  timestamp: number | string | null | undefined,
): string {
  return formatBaseLegacyDateMinuteSlash(timestamp);
}
