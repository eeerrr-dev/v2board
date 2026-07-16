import type { ReactElement, ReactNode } from 'react';
import { render, type RenderOptions, type RenderResult } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { createMemoryRouter, MemoryRouter, RouterProvider, type RouteObject } from 'react-router';
import { createI18n } from '@v2board/i18n/testing';

export type UserEvent = ReturnType<typeof userEvent.setup>;

/**
 * Shared Testing Library harness for the user app.
 *
 * Providers are OPT-IN so component tests stay minimal: pass `i18n: true`,
 * `queryClient: true`, or `routerEntries: ['/path']` only when the component
 * under test needs that context. Files that vi.mock('react-i18next') or
 * vi.mock('react-router') should keep doing so and simply not opt in.
 */

/** Fresh per-test QueryClient mirroring the app's retry/refetch defaults. */
export function createTestQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { refetchOnWindowFocus: false, retry: false },
    },
  });
}

export interface ProviderOptions {
  /** Wrap in I18nextProvider. `true` builds a fresh createI18n() (zh-CN). */
  i18n?: boolean | I18nInstance;
  /** Wrap in QueryClientProvider. `true` builds a fresh retry-less client. */
  queryClient?: boolean | QueryClient;
  /** Wrap in a MemoryRouter seeded with these entries, e.g. ['/dashboard']. */
  routerEntries?: string[];
}

export interface RenderWithProvidersOptions
  extends Omit<RenderOptions, 'wrapper'>, ProviderOptions {}

export interface RenderWithProvidersResult extends RenderResult {
  /** The I18nInstance in use when `i18n` was requested. */
  i18n?: I18nInstance;
  /** The QueryClient in use when `queryClient` was requested. */
  queryClient?: QueryClient;
  /** A ready userEvent session (created before render, per TL guidance). */
  user: UserEvent;
}

export function renderWithProviders(
  ui: ReactElement,
  options: RenderWithProvidersOptions = {},
): RenderWithProvidersResult {
  const { i18n, queryClient, routerEntries, ...renderOptions } = options;
  const user = userEvent.setup();
  const client = queryClient === true ? createTestQueryClient() : queryClient || undefined;
  const i18nInstance = i18n === true ? createI18n() : i18n || undefined;

  // Provider order mirrors main.tsx: i18n > query > router.
  function Wrapper({ children }: { children: ReactNode }) {
    let tree = children;
    if (routerEntries) tree = <MemoryRouter initialEntries={routerEntries}>{tree}</MemoryRouter>;
    if (client) tree = <QueryClientProvider client={client}>{tree}</QueryClientProvider>;
    if (i18nInstance) tree = <I18nextProvider i18n={i18nInstance}>{tree}</I18nextProvider>;
    return <>{tree}</>;
  }

  return {
    ...render(ui, { ...renderOptions, wrapper: Wrapper }),
    i18n: i18nInstance,
    queryClient: client,
    user,
  };
}

export interface RenderRoutesOptions extends RenderWithProvidersOptions {
  /** Initial history stack for the memory data router. Defaults to ['/']. */
  initialEntries?: string[];
}

export interface RenderRoutesResult extends RenderWithProvidersResult {
  router: ReturnType<typeof createMemoryRouter>;
}

/**
 * Data-router variant (createMemoryRouter + RouterProvider) for components
 * that need loaders, actions, useParams, or router.navigate in assertions.
 */
export function renderRoutes(
  routes: RouteObject[],
  options: RenderRoutesOptions = {},
): RenderRoutesResult {
  const { initialEntries = ['/'], ...rest } = options;
  const router = createMemoryRouter(routes, { initialEntries });
  const result = renderWithProviders(<RouterProvider router={router} />, rest);
  return { ...result, router };
}
