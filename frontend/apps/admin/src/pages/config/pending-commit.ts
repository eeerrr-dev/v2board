import { AUTH_KEY } from '@/lib/auth';
import { SECTIONS, type ConfigGroupKey } from './schema';

const SCHEMA_VERSION = 2;
const SESSION_STORAGE_KEY = 'v2board.admin.pending-config.v2';
const LOCAL_FALLBACK_STORAGE_KEY = 'v2board.admin.pending-config-fallback.v2';
const PENDING_COMMIT_TTL_MS = 30 * 60 * 1_000;
const MEMORY_ONLY_SCOPE = 'memory-only';
const GROUPS = new Set<ConfigGroupKey>(SECTIONS.map(({ key }) => key));
const listeners = new Set<() => void>();

interface StoredPendingConfigCommit {
  version: typeof SCHEMA_VERSION;
  expiresAt: number;
  identityScope: string;
  commit: PendingConfigCommit;
}

interface StorageRead {
  available: boolean;
  invalid: boolean;
  envelope: StoredPendingConfigCommit | null;
}

let memoryEnvelope: StoredPendingConfigCommit | null = null;
let lastKnownIdentityScope: string | null = null;
let suppressedEnvelope: StoredPendingConfigCommit | null = null;
let storageListenerInstalled = false;

/**
 * Durable-write metadata only. PATCH values and secrets are deliberately
 * excluded because browser storage is not a secret store. `securePath` is the
 * one explicit, non-secret routing-coordinate exception: after the dynamic
 * prefix flips, it is required to probe the new endpoint and recover safely.
 */
export interface PendingConfigCommit {
  group: ConfigGroupKey;
  revision: number;
  securePath?: string;
}

export function readPendingConfigCommit(): PendingConfigCommit | null {
  const identityScope = currentIdentityScope();
  if (identityScope === undefined) return currentMemoryCommit();
  if (identityScope === null) {
    discardEverywhere();
    return null;
  }
  lastKnownIdentityScope = identityScope;

  const now = Date.now();
  const session = rejectSuppressedCommit(
    readStoredCommit(window.sessionStorage, SESSION_STORAGE_KEY, identityScope, now),
    window.sessionStorage,
    SESSION_STORAGE_KEY,
  );
  const fallback = rejectSuppressedCommit(
    readStoredCommit(window.localStorage, LOCAL_FALLBACK_STORAGE_KEY, identityScope, now),
    window.localStorage,
    LOCAL_FALLBACK_STORAGE_KEY,
  );
  const resolved = resolveStoredEnvelopes(session.envelope, fallback.envelope);
  if (resolved) {
    const conflicted = sameRevisionConflict(session.envelope, fallback.envelope);
    if (!conflicted) {
      if (!sameEnvelope(resolved, session.envelope)) {
        writeStorage(window.sessionStorage, SESSION_STORAGE_KEY, resolved);
      }
      if (!sameEnvelope(resolved, fallback.envelope)) {
        writeStorage(window.localStorage, LOCAL_FALLBACK_STORAGE_KEY, resolved);
      }
    }
    return remember(resolved);
  }

  if (session.invalid || fallback.invalid) return remember(null);
  if (!session.available || !fallback.available) return currentMemoryCommit(identityScope, now);
  return remember(null);
}

export function writePendingConfigCommit(commit: PendingConfigCommit): void {
  const currentScope = currentIdentityScope();
  const identityScope =
    currentScope === undefined
      ? (lastKnownIdentityScope ?? MEMORY_ONLY_SCOPE)
      : (currentScope ?? MEMORY_ONLY_SCOPE);
  if (currentScope === null) lastKnownIdentityScope = null;
  else if (currentScope !== undefined) lastKnownIdentityScope = currentScope;

  const envelope: StoredPendingConfigCommit = {
    version: SCHEMA_VERSION,
    expiresAt: Date.now() + PENDING_COMMIT_TTL_MS,
    identityScope,
    commit,
  };
  suppressedEnvelope = null;

  // sessionStorage is the primary store. A second, identity-scoped and
  // short-lived copy makes an already-durable server commit recoverable when
  // browsers deny or later break sessionStorage. Neither copy contains the
  // PATCH body, secrets, or values other than the required secure-path routing
  // coordinate documented above.
  if (identityScope !== MEMORY_ONLY_SCOPE) {
    writeStorage(window.sessionStorage, SESSION_STORAGE_KEY, envelope);
    writeStorage(window.localStorage, LOCAL_FALLBACK_STORAGE_KEY, envelope);
  }
  remember(envelope);
  emitPendingConfigCommitChange();
}

export function clearPendingConfigCommit(expected?: PendingConfigCommit): boolean {
  const active = readPendingConfigCommit();
  if (expected && !samePendingCommit(active, expected)) return false;

  suppressedEnvelope = active ? memoryEnvelope : null;
  removeStorage(window.sessionStorage, SESSION_STORAGE_KEY);
  removeStorage(window.localStorage, LOCAL_FALLBACK_STORAGE_KEY);
  remember(null);
  emitPendingConfigCommitChange();
  return true;
}

export function subscribePendingConfigCommit(listener: () => void): () => void {
  listeners.add(listener);
  if (!storageListenerInstalled) {
    window.addEventListener('storage', handleFallbackStorageChange);
    storageListenerInstalled = true;
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && storageListenerInstalled) {
      window.removeEventListener('storage', handleFallbackStorageChange);
      storageListenerInstalled = false;
    }
  };
}

function readStoredCommit(
  storage: Storage,
  key: string,
  identityScope: string,
  now: number,
): StorageRead {
  let raw: string | null;
  try {
    raw = storage.getItem(key);
  } catch {
    return { available: false, invalid: false, envelope: null };
  }
  if (!raw) return { available: true, invalid: false, envelope: null };

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    removeStorage(storage, key);
    return { available: true, invalid: true, envelope: null };
  }
  if (!isStoredPendingConfigCommit(parsed, identityScope, now)) {
    removeStorage(storage, key);
    return { available: true, invalid: true, envelope: null };
  }
  return { available: true, invalid: false, envelope: parsed };
}

function rejectSuppressedCommit(read: StorageRead, storage: Storage, key: string): StorageRead {
  if (!read.envelope || !suppressedEnvelope) return read;
  if (!sameEnvelope(read.envelope, suppressedEnvelope)) return read;
  removeStorage(storage, key);
  return { available: read.available, invalid: true, envelope: null };
}

function resolveStoredEnvelopes(
  session: StoredPendingConfigCommit | null,
  fallback: StoredPendingConfigCommit | null,
): StoredPendingConfigCommit | null {
  if (!session) return fallback;
  if (!fallback || sameEnvelope(session, fallback)) return session;
  if (session.commit.revision !== fallback.commit.revision) {
    return session.commit.revision > fallback.commit.revision ? session : fallback;
  }

  // Equal revisions should identify the same durable backend commit. If the
  // metadata conflicts, preserve a deterministic locked snapshot but do not
  // overwrite either source: a later server observation (or the TTL) must
  // resolve it, rather than silently downgrading one tab's state.
  if (session.expiresAt !== fallback.expiresAt) {
    return session.expiresAt > fallback.expiresAt ? session : fallback;
  }
  return canonicalCommitKey(session.commit) >= canonicalCommitKey(fallback.commit)
    ? session
    : fallback;
}

function sameRevisionConflict(
  session: StoredPendingConfigCommit | null,
  fallback: StoredPendingConfigCommit | null,
): boolean {
  return Boolean(
    session &&
    fallback &&
    session.commit.revision === fallback.commit.revision &&
    !samePendingCommit(session.commit, fallback.commit),
  );
}

function canonicalCommitKey(commit: PendingConfigCommit): string {
  return `${commit.group}\u0000${commit.securePath ?? ''}`;
}

function handleFallbackStorageChange(event: StorageEvent): void {
  if (event.storageArea && event.storageArea !== window.localStorage) return;
  if (event.key !== LOCAL_FALLBACK_STORAGE_KEY && event.key !== null) return;
  emitPendingConfigCommitChange();
}

function isStoredPendingConfigCommit(
  value: unknown,
  identityScope: string,
  now: number,
): value is StoredPendingConfigCommit {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ['version', 'expiresAt', 'identityScope', 'commit'])
  ) {
    return false;
  }
  if (
    value.version !== SCHEMA_VERSION ||
    typeof value.expiresAt !== 'number' ||
    !Number.isSafeInteger(value.expiresAt) ||
    value.expiresAt <= now ||
    value.expiresAt > now + PENDING_COMMIT_TTL_MS ||
    value.identityScope !== identityScope ||
    !isRecord(value.commit) ||
    !hasOnlyKeys(value.commit, ['group', 'revision', 'securePath'])
  ) {
    return false;
  }

  const { group, revision, securePath } = value.commit;
  return (
    typeof group === 'string' &&
    GROUPS.has(group as ConfigGroupKey) &&
    typeof revision === 'number' &&
    Number.isSafeInteger(revision) &&
    revision > 0 &&
    (securePath === undefined ||
      (typeof securePath === 'string' && /^[A-Za-z0-9_-]{8,}$/.test(securePath)))
  );
}

function currentIdentityScope(): string | null | undefined {
  let authorization: string | null;
  try {
    authorization = window.localStorage.getItem(AUTH_KEY);
  } catch {
    return undefined;
  }
  if (!authorization) return null;

  // This deterministic FNV-1a fingerprint is only a storage namespace/scope
  // marker. It is deliberately not presented as a password or security hash;
  // authorization remains entirely server-enforced. The raw token is never
  // copied into the pending-commit key or value.
  let fingerprint = 0x811c9dc5;
  for (let index = 0; index < authorization.length; index += 1) {
    fingerprint ^= authorization.charCodeAt(index);
    fingerprint = Math.imul(fingerprint, 0x01000193);
  }
  return `fnv1a32:${(fingerprint >>> 0).toString(16).padStart(8, '0')}`;
}

function currentMemoryCommit(identityScope?: string, now = Date.now()): PendingConfigCommit | null {
  if (!memoryEnvelope || memoryEnvelope.expiresAt <= now) return remember(null);
  if (
    identityScope !== undefined &&
    memoryEnvelope.identityScope !== MEMORY_ONLY_SCOPE &&
    memoryEnvelope.identityScope !== identityScope
  ) {
    return remember(null);
  }
  return memoryEnvelope.commit;
}

function writeStorage(storage: Storage, key: string, envelope: StoredPendingConfigCommit): boolean {
  try {
    storage.setItem(key, JSON.stringify(envelope));
    return true;
  } catch {
    return false;
  }
}

function removeStorage(storage: Storage, key: string): boolean {
  try {
    storage.removeItem(key);
    return true;
  } catch {
    return false;
  }
}

function discardEverywhere(): void {
  removeStorage(window.sessionStorage, SESSION_STORAGE_KEY);
  removeStorage(window.localStorage, LOCAL_FALLBACK_STORAGE_KEY);
  lastKnownIdentityScope = null;
  remember(null);
}

function remember(envelope: StoredPendingConfigCommit | null): PendingConfigCommit | null {
  if (sameEnvelope(memoryEnvelope, envelope)) return memoryEnvelope?.commit ?? null;
  memoryEnvelope = envelope;
  return envelope?.commit ?? null;
}

function emitPendingConfigCommitChange(): void {
  for (const listener of listeners) listener();
}

function samePendingCommit(left: PendingConfigCommit | null, right: PendingConfigCommit): boolean {
  return (
    left?.group === right.group &&
    left.revision === right.revision &&
    left.securePath === right.securePath
  );
}

function sameEnvelope(
  left: StoredPendingConfigCommit | null,
  right: StoredPendingConfigCommit | null,
): boolean {
  return (
    left?.version === right?.version &&
    left?.expiresAt === right?.expiresAt &&
    left?.identityScope === right?.identityScope &&
    (left === null || right === null || samePendingCommit(left.commit, right.commit))
  );
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
