import {
  formatLegacyDateMinuteSlash as formatBaseLegacyDateMinuteSlash,
  formatLegacyDateTime as formatBaseLegacyDateTime,
  legacyEpochDate,
  padLegacyDatePart,
} from '@v2board/config/format';

export function formatUserLegacyDate(
  timestamp: number | string | null | undefined,
): string {
  const date = legacyEpochDate(timestamp);
  if (!date) return 'Invalid date';
  return `${date.getFullYear()}-${padLegacyDatePart(date.getMonth() + 1)}-${padLegacyDatePart(date.getDate())}`;
}

export function formatUserLegacyDateSlash(
  timestamp: number | string | null | undefined,
): string {
  const date = legacyEpochDate(timestamp);
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
