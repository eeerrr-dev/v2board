// The ordinary source run needs no global fixture. In the explicit legacy lane,
// read runtime settings once, start the frozen oracle in-process, and publish
// its base URL to workers; the returned callback closes it after the suite.
import { mkdirSync, writeFileSync } from 'node:fs';
import {
  legacyOracleEnabled,
  oracleHost,
  oraclePort,
  oracleStateRoot,
  publicOracleHost,
} from './lib/env.mjs';
import { readSourceSettings, startOracleServer } from './lib/oracle-server.mjs';
import { oracleUrlFile } from './lib/oracle-url.mjs';

export default async function globalSetup() {
  // Product-owned source behavior is the standing gate. The frozen oracle is
  // started only by the explicit legacy-oracle-parity migration lane.
  if (!legacyOracleEnabled) return undefined;

  const sourceSettings = await readSourceSettings();
  const oracle = await startOracleServer(oraclePort, oracleHost, publicOracleHost, sourceSettings);

  mkdirSync(oracleStateRoot, { recursive: true });
  writeFileSync(oracleUrlFile, oracle.baseUrl.toString());
  console.log(`Parity oracle listening at ${oracle.baseUrl}`);

  return async () => {
    await oracle.close();
  };
}
