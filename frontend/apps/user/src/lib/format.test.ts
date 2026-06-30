import { describe, expect, it } from 'vitest';
import {
  formatBytes,
  formatDateTime,
  formatLegacyDate,
  formatLegacyDateMinuteSlash,
  formatLegacyDateSlash,
  formatLegacyDateTime,
  formatMoney,
} from '@v2board/config/format';

describe('formatters', () => {
  it('formats bytes', () => {
    expect(formatBytes(-1)).toBe(0);
    expect(formatBytes(0)).toBe('0.00 B');
    expect(formatBytes(1024)).toBe('1024.00 B');
    expect(formatBytes(1025)).toBe('1.00 KB');
    expect(formatBytes(1024 * 1024)).toBe('1024.00 KB');
    expect(formatBytes(1024 * 1024 + 1)).toBe('1.00 MB');
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1024.00 MB');
    expect(formatBytes(2 * 1024 * 1024 * 1024)).toBe('2.00 GB');
  });
  it('formats money in cents', () => {
    expect(formatMoney(12345)).toBe('¥123.45');
    expect(formatMoney(50, '$')).toBe('$0.50');
  });
  it('formats date time', () => {
    expect(formatDateTime(null)).toBe('-');
    const t = Math.floor(new Date('2026-05-23T00:00:00Z').getTime() / 1000);
    const s = formatDateTime(t);
    expect(s).toMatch(/2026-05-2[23] \d{2}:\d{2}:\d{2}/);
  });
  it('formats the legacy date family with numeric coercion', () => {
    // Numeric dayjs format tokens are locale-independent; coerce strings and
    // fall back to "Invalid date" for non-timestamps. Midday UTC keeps the date
    // stable across the runner's timezone.
    const t = Math.floor(new Date('2026-05-23T12:00:00Z').getTime() / 1000);
    expect(formatLegacyDate(undefined)).toBe('Invalid date');
    expect(formatLegacyDateSlash(undefined)).toBe('Invalid date');
    expect(formatLegacyDate(t)).toBe('2026-05-23');
    expect(formatLegacyDate(String(t))).toBe('2026-05-23');
    expect(formatLegacyDateSlash(t)).toBe('2026/05/23');
    expect(formatLegacyDateMinuteSlash(t)).toMatch(/2026\/05\/23 \d{2}:\d{2}/);
    expect(formatLegacyDateTime(t)).toMatch(/2026-05-23 \d{2}:\d{2}:\d{2}/);
  });
});
