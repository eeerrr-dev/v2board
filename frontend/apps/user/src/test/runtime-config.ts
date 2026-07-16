import { resetRuntimeConfigForTests, type RuntimeConfig } from '@/lib/runtime-config';

export function setRuntimeConfig(config?: RuntimeConfig): void {
  resetRuntimeConfigForTests();
  document.getElementById('v2board-runtime-config')?.remove();
  if (config === undefined) return;
  const element = document.createElement('script');
  element.id = 'v2board-runtime-config';
  element.type = 'application/json';
  element.textContent = JSON.stringify(config);
  document.head.append(element);
}
