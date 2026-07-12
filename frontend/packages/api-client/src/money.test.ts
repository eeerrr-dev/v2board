import { describe, expect, it } from 'vitest';
import { decimalToCents, decimalToMinorUnits, decimalToScaledInteger } from './money';

describe('decimalToMinorUnits', () => {
  it.each([
    ['19.99', 1999],
    ['0.1', 10],
    ['0.005', 1],
    ['-0.005', -1],
    [19.99, 1999],
  ])('converts %s without floating-point drift', (value, expected) => {
    expect(decimalToCents(value)).toBe(expected);
  });

  it('supports currencies with a different minor-unit scale', () => {
    expect(decimalToMinorUnits('12.3456', 3)).toBe(12346);
  });

  it.each(['', 'NaN', '1e3', '12 dollars'])('rejects invalid amount %j', (value) => {
    expect(() => decimalToCents(value)).toThrow(TypeError);
  });

  it('rejects unsafe integer results', () => {
    expect(() => decimalToCents('9007199254740991')).toThrow(RangeError);
  });
});

describe('decimalToScaledInteger', () => {
  it.each([
    ['1.5', 1_073_741_824, 1_610_612_736],
    ['0.0000000004656612873077392578125', 1_073_741_824, 1],
    ['-0.005', 100, -1],
    ['12.34', 100, 1234],
  ])('scales %s by %s exactly', (value, scale, expected) => {
    expect(decimalToScaledInteger(value, scale)).toBe(expected);
  });

  it('rounds a fractional scaled unit half away from zero', () => {
    expect(decimalToScaledInteger('0.5', 1)).toBe(1);
    expect(decimalToScaledInteger('-0.5', 1)).toBe(-1);
  });

  it('rejects invalid scales and unsafe results', () => {
    expect(() => decimalToScaledInteger('1', 0)).toThrow(RangeError);
    expect(() => decimalToScaledInteger('1', Number.MAX_SAFE_INTEGER + 1)).toThrow(RangeError);
    expect(() => decimalToScaledInteger('9007199254740992', 1)).toThrow(RangeError);
  });
});
