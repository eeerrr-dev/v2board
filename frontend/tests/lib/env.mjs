import { resolve } from 'node:path';

export const sourceBaseUrl = new URL(
  process.env.VISUAL_PARITY_SOURCE_BASE_URL ?? 'http://rust-api:8080',
);
export const adminPath = stripSlashes(process.env.VISUAL_PARITY_ADMIN_PATH ?? 'admin');
export const referenceRoot = resolve(process.env.REFERENCE_ORACLE_ROOT ?? '/reference');
export const oraclePublicRoot = resolve(referenceRoot, 'public');
export const oracleStateRoot = resolve(
  process.env.REFERENCE_ORACLE_STATE_ROOT ?? '/tmp/v2board-reference-oracle',
);
export const oracleHost = process.env.VISUAL_PARITY_ORACLE_HOST ?? '127.0.0.1';
export const publicOracleHost = process.env.VISUAL_PARITY_PUBLIC_ORACLE_HOST ?? oracleHost;
export const oraclePort = Number(process.env.VISUAL_PARITY_ORACLE_PORT ?? '0');
export const navigationAttempts = Number(process.env.VISUAL_PARITY_NAVIGATION_ATTEMPTS ?? '3');
export const navigationTimeout = Number(process.env.VISUAL_PARITY_NAVIGATION_TIMEOUT ?? '45000');
export const fontWaitTimeout = Number(process.env.VISUAL_PARITY_FONT_WAIT_TIMEOUT ?? '5000');
export const LEGACY_GB_BYTES = 1_073_741_824;
export const viewports = [
  { height: 900, label: 'desktop', width: 1440 },
  { height: 844, label: 'mobile', width: 390 },
];

export function stripSlashes(value) {
  return value.trim().replace(/^\/+|\/+$/g, '');
}
