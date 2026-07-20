import type { ReactElement } from 'react';
import { render, type RenderOptions, type RenderResult } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

export interface RenderWithUserResult extends RenderResult {
  user: ReturnType<typeof userEvent.setup>;
}

/** Package-local render helper; shared primitives must not depend on either app harness. */
export function renderWithUser(
  ui: ReactElement,
  options: RenderOptions = {},
): RenderWithUserResult {
  const user = userEvent.setup();
  return { ...render(ui, options), user };
}
