import dayjs from 'dayjs';
import {
  formatLegacyDateMinuteSlash as formatBaseLegacyDateMinuteSlash,
  formatLegacyDateTime as formatBaseLegacyDateTime,
} from '@v2board/config/format';

export function formatUserLegacyDate(
  timestamp: number | string | null | undefined,
): string {
  const date = dayjs(Number(timestamp) * 1000);
  if (!date.isValid()) return 'Invalid date';
  return date.format('YYYY-MM-DD');
}

export function formatUserLegacyDateSlash(
  timestamp: number | string | null | undefined,
): string {
  const date = dayjs(Number(timestamp) * 1000);
  if (!date.isValid()) return 'Invalid date';
  return date.format('YYYY/MM/DD');
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
