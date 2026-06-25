import { afterEach, describe, expect, it } from 'vitest';
import {
  formatLegacyDateMinuteSlash,
  formatLegacyDateTime,
} from '@v2board/config/format';
import {
  formatUserLegacyDate,
  formatUserLegacyDateMinuteSlash,
  formatUserLegacyDateSlash,
  formatUserLegacyDateTime,
} from './legacy-date';

describe('user legacy date formatting', () => {
  afterEach(() => {
    window.localStorage.removeItem('umi_locale');
  });

  it('keeps legacy dates byte-for-byte with the shared formatter', () => {
    window.localStorage.setItem('umi_locale', 'zh-CN');

    expect(formatUserLegacyDateMinuteSlash(1_700_000_000)).toBe(
      formatLegacyDateMinuteSlash(1_700_000_000),
    );
    expect(formatUserLegacyDate(1_700_000_000)).toBe('2023-11-14');
    expect(formatUserLegacyDateSlash(1_700_000_000)).toBe('2023/11/14');
    expect(formatUserLegacyDateTime(1_700_000_000)).toBe(
      formatLegacyDateTime(1_700_000_000),
    );
  });
});
