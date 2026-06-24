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
  return legacyLocalePostformat(
    `${date.getFullYear()}-${padLegacyDatePart(date.getMonth() + 1)}-${padLegacyDatePart(date.getDate())}`,
  );
}

export function formatUserLegacyDateSlash(
  timestamp: number | string | null | undefined,
): string {
  const date = legacyDate(timestamp);
  if (!date) return 'Invalid date';
  return legacyLocalePostformat(
    `${date.getFullYear()}/${padLegacyDatePart(date.getMonth() + 1)}/${padLegacyDatePart(date.getDate())}`,
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
