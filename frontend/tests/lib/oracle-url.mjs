// Bridge the dynamically-bound oracle base URL from globalSetup (runner process)
// to the worker processes. Playwright forks workers separately, so a value set
// on process.env in globalSetup never reaches them; a tiny file keyed off the
// shared writable state directory does, and it also tolerates the OS-assigned port 0.
import { resolve } from 'node:path';
import { oracleStateRoot } from './env.mjs';

export const oracleUrlFile = resolve(oracleStateRoot, 'oracle-base-url.txt');
