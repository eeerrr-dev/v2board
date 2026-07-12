#!/usr/bin/env node
// Serve the read-only reference UI for focused compatibility inspection. Assets
// come only from the pinned project under references/ and never enter a build.
import { adminPath, oracleHost, oraclePort, publicOracleHost } from '../tests/lib/env.mjs';
import { readSourceSettings, startOracleServer, waitForShutdown } from '../tests/lib/oracle-server.mjs';

const sourceSettings = await readSourceSettings();
const server = await startOracleServer(oraclePort, oracleHost, publicOracleHost, sourceSettings);

console.log(`Reference UI user: ${new URL('/', server.baseUrl)}`);
console.log(`Reference UI admin: ${new URL(`/${adminPath}#/login`, server.baseUrl)}`);
console.log('Press Ctrl-C to stop.');

await waitForShutdown();
await server.close();
process.exit(0);
