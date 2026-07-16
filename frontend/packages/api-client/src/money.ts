function parseDecimal(value: string | number) {
  const input = String(value).trim();
  const match = /^([+-]?)(\d+)(?:\.(\d*))?$/.exec(input);
  if (!match) throw new TypeError(`Invalid decimal amount: ${input || '<empty>'}`);

  return {
    negative: match[1] === '-',
    integer: match[2] ?? '0',
    fraction: match[3] ?? '',
  };
}

/**
 * Multiply a user-entered decimal by an integer scale without passing through
 * binary floating point. Fractional results are rounded half away from zero.
 *
 * This covers both decimal minor units (`scale = 100`) and non-decimal units
 * such as GiB (`scale = 1_073_741_824`).
 */
export function decimalToScaledInteger(value: string | number, scale: number | bigint): number {
  const factor = typeof scale === 'bigint' ? scale : BigInt(scale);
  if (factor <= 0n || (typeof scale === 'number' && !Number.isSafeInteger(scale))) {
    throw new RangeError('scale must be a positive safe integer');
  }

  const { negative, integer, fraction } = parseDecimal(value);
  const denominator = 10n ** BigInt(fraction.length);
  const decimal = BigInt(`${integer}${fraction}`);
  const numerator = decimal * factor;
  let scaled = numerator / denominator;
  const remainder = numerator % denominator;
  if (remainder * 2n >= denominator) scaled += 1n;
  if (negative) scaled = -scaled;

  const result = Number(scaled);
  if (!Number.isSafeInteger(result)) {
    throw new RangeError('Scaled value exceeds the safe integer range');
  }
  return result;
}

/**
 * Convert a user-entered decimal amount to integer minor units without binary
 * floating-point multiplication. Values with more than `scale` fractional
 * digits are rounded half away from zero.
 */
export function decimalToMinorUnits(value: string | number, scale = 2): number {
  if (!Number.isInteger(scale) || scale < 0 || scale > 8) {
    throw new RangeError('scale must be an integer between 0 and 8');
  }
  return decimalToScaledInteger(value, 10n ** BigInt(scale));
}

export function decimalToCents(value: string | number): number {
  return decimalToMinorUnits(value, 2);
}
