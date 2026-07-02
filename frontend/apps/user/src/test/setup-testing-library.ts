import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// Vitest runs with globals:false, so Testing Library's self-registration
// (which probes for a global afterEach/beforeAll) never fires. Register the
// per-test DOM cleanup and the React act() environment explicitly. The legacy
// hand-rolled createRoot harnesses set IS_REACT_ACT_ENVIRONMENT themselves, so
// enabling it globally is compatible with both harnesses during the migration.
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

afterEach(() => {
  cleanup();
});
