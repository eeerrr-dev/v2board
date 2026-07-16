import { resetRuntimeConfigForTests, type AdminRuntimeConfig } from '@/lib/runtime-config';

export function setAdminRuntimeConfig(config?: AdminRuntimeConfig): void {
  resetRuntimeConfigForTests();
  document.getElementById('v2board-runtime-config')?.remove();
  if (config === undefined) return;
  const element = document.createElement('script');
  element.id = 'v2board-runtime-config';
  element.type = 'application/json';
  element.textContent = JSON.stringify(config);
  document.head.append(element);
}
