// Playwright globalSetup: read the live source app's runtime settings once, start
// the frozen antd oracle HTTP server in the runner process, and publish its base
// URL to workers via the oracle-url file. The returned callback runs as global
// teardown, closing the server after the whole suite.
import { mkdirSync, writeFileSync } from 'node:fs';
import { oracleHost, oraclePort, oracleRoot, publicOracleHost } from './lib/env.mjs';
import { readSourceSettings, startOracleServer } from './lib/oracle-server.mjs';
import { oracleUrlFile } from './lib/oracle-url.mjs';

export default async function globalSetup() {
  const sourceSettings = await readSourceSettings();
  const oracle = await startOracleServer(oraclePort, oracleHost, publicOracleHost, sourceSettings);

  mkdirSync(oracleRoot, { recursive: true });
  writeFileSync(oracleUrlFile, oracle.baseUrl.toString());
  console.log(`Parity oracle listening at ${oracle.baseUrl}`);

  return async () => {
    await oracle.close();
  };
}
