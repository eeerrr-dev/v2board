import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const RELEASE_ID_HEX_LENGTH = 20;

function updateFramed(digest, value) {
  const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(BigInt(bytes.length));
  digest.update(length);
  digest.update(bytes);
}

function assertSafeRelativeFile(path) {
  if (
    typeof path !== 'string' ||
    path.length === 0 ||
    path.startsWith('/') ||
    path.includes('\\') ||
    path.split('/').some((segment) => !segment || segment === '.' || segment === '..')
  ) {
    throw new Error(`Release content path is not canonical and relative: ${path}`);
  }
}

export async function releaseIdFromBuilds(builds) {
  const digest = createHash('sha256');
  digest.update('v2board-frontend-release-content-v1\0');

  for (const build of builds) {
    if (typeof build.name !== 'string' || !/^[a-z][a-z0-9-]*$/.test(build.name)) {
      throw new Error(`Release build name is invalid: ${build.name}`);
    }
    const files = [...build.files].sort();
    if (new Set(files).size !== files.length) {
      throw new Error(`Release build ${build.name} contains duplicate paths`);
    }
    updateFramed(digest, build.name);
    updateFramed(digest, String(files.length));
    for (const path of files) {
      assertSafeRelativeFile(path);
      updateFramed(digest, path);
      updateFramed(digest, await readFile(join(build.outDir, path)));
    }
  }

  return digest.digest('hex').slice(0, RELEASE_ID_HEX_LENGTH);
}
