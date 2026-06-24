import {
  formatLegacyDateMinuteSlash as formatBaseLegacyDateMinuteSlash,
  formatLegacyDateTime as formatBaseLegacyDateTime,
} from '@v2board/config/format';
import { legacyGetLocale } from '@v2board/i18n';

const PERSIAN_DIGITS = ['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹'];

function legacyLocalePostformat(value: string): string {
  if (legacyGetLocale() !== 'fa-IR') return value;
  return value.replace(/\d/g, (digit) => PERSIAN_DIGITS[Number(digit)] ?? digit);
}

export function formatUserLegacyDateSlash(
  timestamp: number | string | null | undefined,
): string {
  const date = new Date(Number(timestamp) * 1000);
  if (Number.isNaN(date.getTime())) return 'Invalid date';
  const pad = (value: number) => `${value}`.padStart(2, '0');
  return legacyLocalePostformat(
    `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())}`,
  );
}

export function formatUserLegacyDateTime(
  timestamp: number | string | null | undefined,
): string {
  return legacyLocalePostformat(formatBaseLegacyDateTime(timestamp));
}

export function formatUserLegacyDateMinuteSlash(
  timestamp: number | string | null | undefined,
): string {
  return legacyLocalePostformat(formatBaseLegacyDateMinuteSlash(timestamp));
}
