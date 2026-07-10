#!/usr/bin/env node
// Serve the frozen antd oracle for manual inspection (make legacy-oracle-up /
// legacy-oracle-serve). This is the old visual-parity.mjs SERVE_ONLY path,
// rebuilt on the extracted oracle server so the 20k-line driver can retire.
import { adminPath, oracleHost, oraclePort, publicOracleHost } from '../tests/lib/env.mjs';
import { readSourceSettings, startOracleServer, waitForShutdown } from '../tests/lib/oracle-server.mjs';

const sourceSettings = await readSourceSettings();
const server = await startOracleServer(oraclePort, oracleHost, publicOracleHost, sourceSettings);

console.log(`Legacy oracle user: ${new URL('/', server.baseUrl)}`);
console.log(`Legacy oracle admin: ${new URL(`/${adminPath}#/login`, server.baseUrl)}`);
console.log('Press Ctrl-C to stop.');

await waitForShutdown();
await server.close();
process.exit(0);
