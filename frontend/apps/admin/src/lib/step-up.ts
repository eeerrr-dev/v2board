import { isStepUpRequiredProblem } from '@v2board/api-client';

// Privileged step-up grant (POST /auth/step-up). The token is bound
// server-side to the current session and rides on requests as the
// `x-v2board-step-up` header. Held in memory only: after a reload the backend
// either still honors the session's recent-password window or asks again.

interface StepUpGrant {
  token: string;
  expiresAt: number;
}

let grant: StepUpGrant | null = null;

// Drop the token slightly before `expires_in` lapses: the backend rejects a
// stale token outright instead of falling back to the recent-password window.
const EXPIRY_MARGIN_MS = 5_000;

export function getStepUpToken(): string | null {
  if (!grant) return null;
  if (Date.now() >= grant.expiresAt - EXPIRY_MARGIN_MS) {
    grant = null;
    return null;
  }
  return grant.token;
}

export function setStepUpGrant(token: string, expiresInSeconds: number): void {
  grant = { token, expiresAt: Date.now() + expiresInSeconds * 1000 };
}

export function clearStepUpGrant(): void {
  grant = null;
}

// Imperative prompt request, exposed as an external store the same way
// confirm-dialog.tsx does, so the MutationCache/QueryCache error hooks can
// open the dialog from outside React.

let promptRequested = false;
const listeners = new Set<() => void>();

function notify(): void {
  listeners.forEach((listener) => listener());
}

export function subscribeStepUpPrompt(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function isStepUpPromptRequested(): boolean {
  return promptRequested;
}

export function resolveStepUpPrompt(): void {
  if (!promptRequested) return;
  promptRequested = false;
  notify();
}

/**
 * Open the re-auth dialog when `error` is the backend's step-up 403 problem
 * (`code: "step_up_required"`). Returns true when the error was consumed so
 * callers can skip their generic error presentation.
 */
export function maybePromptStepUp(error: unknown): boolean {
  if (!isStepUpRequiredProblem(error)) return false;
  if (!promptRequested) {
    promptRequested = true;
    notify();
  }
  return true;
}
