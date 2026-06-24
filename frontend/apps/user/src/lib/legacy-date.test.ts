import { afterEach, describe, expect, it } from 'vitest';
import {
  formatLegacyDateMinuteSlash,
  formatLegacyDateTime,
} from '@v2board/config/format';
import {
  formatUserLegacyDateMinuteSlash,
  formatUserLegacyDateSlash,
  formatUserLegacyDateTime,
} from './legacy-date';

const PERSIAN_DIGITS = ['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹'];

function toPersianDigits(value: string): string {
  return value.replace(/\d/g, (digit) => PERSIAN_DIGITS[Number(digit)] ?? digit);
}

describe('user legacy date locale postformat', () => {
  afterEach(() => {
    window.localStorage.removeItem('umi_locale');
  });

  it('keeps non-fa legacy dates byte-for-byte with the shared formatter', () => {
    window.localStorage.setItem('umi_locale', 'zh-CN');

    expect(formatUserLegacyDateMinuteSlash(1_700_000_000)).toBe(
      formatLegacyDateMinuteSlash(1_700_000_000),
    );
    expect(formatUserLegacyDateSlash(1_700_000_000)).toBe('2023/11/14');
    expect(formatUserLegacyDateTime(1_700_000_000)).toBe(
      formatLegacyDateTime(1_700_000_000),
    );
  });

  it('matches the legacy fa-IR moment postformat by using Persian digits', () => {
    window.localStorage.setItem('umi_locale', 'fa-IR');

    expect(formatUserLegacyDateMinuteSlash(1_700_000_000)).toBe(
      toPersianDigits(formatLegacyDateMinuteSlash(1_700_000_000)),
    );
    expect(formatUserLegacyDateSlash(1_700_000_000)).toBe(
      toPersianDigits('2023/11/14'),
    );
    expect(formatUserLegacyDateTime(1_700_000_000)).toBe(
      toPersianDigits(formatLegacyDateTime(1_700_000_000)),
    );
  });
});
