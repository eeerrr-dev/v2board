const baseUrl = process.env.DEPLOY_SMOKE_BASE_URL ?? 'http://rust-api:8080';
const adminPath = (process.env.DEPLOY_SMOKE_ADMIN_PATH ?? 'admin').replace(/^\/+|\/+$/g, '');

async function requireOk(url, label) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${label} returned ${response.status}`);
  return response;
}

async function verifyShell(path, assetPrefix) {
  const response = await requireOk(new URL(path, baseUrl), path);
  const html = await response.text();
  if (!html.includes('id="root"') || html.includes('__V2BOARD_RUNTIME_CONFIG__')) {
    throw new Error(`${path} did not render a resolved SPA shell`);
  }

  const assets = Array.from(
    html.matchAll(/(?:src|href)="([^"]+)"/g),
    (match) => match[1],
  ).filter((url) => url?.startsWith(assetPrefix));
  if (assets.length < 2) {
    throw new Error(`${path} did not expose hashed JavaScript and CSS URLs`);
  }

  for (const asset of assets) {
    await requireOk(new URL(asset, baseUrl), asset);
  }

  const manifest = await fetch(new URL(`${assetPrefix}manifest.json`, baseUrl));
  if (manifest.status !== 404) {
    throw new Error(`${assetPrefix}manifest.json must not be public`);
  }
}

await requireOk(new URL('/healthz', baseUrl), 'healthz');
await verifyShell('/', '/assets/user/');
await verifyShell(`/${adminPath}`, '/assets/admin/');
console.log('Rust deploy smoke OK');
// The CLI runs in a one-shot Compose container; do not retain undici's pooled sockets.
process.exit(0);
