import { readlink, readdir, rename, rm, stat, symlink } from 'node:fs/promises';
import { basename, join } from 'node:path';

const RELEASE_LINK_PATTERN = /^releases\/[a-f0-9]{20}$/;

export async function publishReleaseLinks(target, releaseName) {
  if (!RELEASE_LINK_PATTERN.test(releaseName)) {
    throw new Error(`Deploy release has an unsafe name: ${releaseName}`);
  }
  const release = await stat(join(target, releaseName));
  if (!release.isDirectory()) {
    throw new Error(`Deploy release is not a directory: ${releaseName}`);
  }

  const current = await readDeployLink(target, 'current');
  if (current !== releaseName) {
    // A first publication has no historical generation. Pointing both links
    // at the same immutable content ID makes that state explicit and gives
    // the runtime a total local fallback tree; it is not evidence of a real
    // prior release or of cross-instance asset retention.
    await replaceDeployLink(target, 'previous', current ?? releaseName);
    await replaceDeployLink(target, 'current', releaseName);
  }

  let previous = await readDeployLink(target, 'previous');
  if (!previous) {
    // Heal deploy roots produced by the pre-contract builder when an
    // idempotent rebuild finds current already published without previous.
    await replaceDeployLink(target, 'previous', releaseName);
    previous = releaseName;
  }
  await pruneOldReleases(
    join(target, 'releases'),
    new Set([releaseName, previous].filter(Boolean)),
  );
  return previous;
}

async function readDeployLink(target, name) {
  const linkPath = join(target, name);
  try {
    const link = await readlink(linkPath);
    if (!RELEASE_LINK_PATTERN.test(link)) {
      throw new Error(`Deploy ${name} link has an unsafe target: ${link}`);
    }
    await stat(join(target, link));
    return link;
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}

async function replaceDeployLink(target, name, linkTarget) {
  const temporary = join(target, `.${name}-${process.pid}-${Date.now()}`);
  await symlink(linkTarget, temporary);
  try {
    await rename(temporary, join(target, name));
  } finally {
    await rm(temporary, { force: true });
  }
}

async function pruneOldReleases(releases, retainedLinks) {
  const retainedNames = new Set([...retainedLinks].map((link) => basename(link)));
  for (const entry of await readdir(releases, { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^[a-f0-9]{20}$/.test(entry.name)) continue;
    if (!retainedNames.has(entry.name)) {
      await rm(join(releases, entry.name), { recursive: true, force: true });
    }
  }
}
