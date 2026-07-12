import { decimalToCents } from '@v2board/api-client';

export type NumberInput = string | number;

export function isBlankInput(value: unknown) {
  return value == null || (typeof value === 'string' && value.trim() === '');
}

export function isEmptyInput(value: unknown) {
  return value == null || value === '';
}

function inputText(value: NumberInput) {
  return String(value).trim();
}

export function isNumericInput(value: unknown): value is NumberInput {
  if (typeof value !== 'string' && typeof value !== 'number') return false;
  const text = inputText(value);
  if (!/^[+-]?\d+(?:\.\d*)?$/.test(text)) return false;
  return Number.isFinite(Number(text));
}

export function isIntegerInput(value: unknown): value is NumberInput {
  if (typeof value !== 'string' && typeof value !== 'number') return false;
  const text = inputText(value);
  if (!/^[+-]?\d+$/.test(text)) return false;
  return Number.isSafeInteger(Number(text));
}

// Money stays in its display-unit representation in forms and is converted at
// the API boundary. This mirrors the accepted decimal grammar there and also
// rejects values whose cents conversion cannot be represented safely.
export function isMoneyInput(value: unknown): value is NumberInput {
  if (!isNumericInput(value)) return false;
  try {
    decimalToCents(value);
    return true;
  } catch {
    return false;
  }
}

export function isHttpUrlInput(value: unknown) {
  if (typeof value !== 'string') return false;
  try {
    const url = new URL(value);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}
