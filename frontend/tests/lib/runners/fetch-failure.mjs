import { fetchFailureState } from '../state-readers/shared.mjs';

export async function runFetchFailureStateInteraction(page) {
  await page.waitForTimeout(500);
  return fetchFailureState(page);
}
