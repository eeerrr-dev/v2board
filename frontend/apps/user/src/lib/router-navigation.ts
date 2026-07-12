// Keep only the data-router capability this boundary owns. React Router's
// DataRouter satisfies it, while tests do not need to counterfeit the entire
// overloaded router surface.
interface RouterNavigation {
  navigate(to: '/login', options: { replace: true }): void | Promise<void>;
}

let router: RouterNavigation | undefined;

export function registerRouterNavigation(nextRouter: RouterNavigation): void {
  router = nextRouter;
}

export function navigateToLogin(): void {
  if (!router) throw new Error('User router navigation was used before bootstrap registration');
  void router.navigate('/login', { replace: true });
}
