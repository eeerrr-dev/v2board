import { normalizeParityText } from './text.mjs';
import { subscribeTargetTitles } from './fixture-data.mjs';

export function pickFetchQueryFields(query, keys) {
  if (!query || typeof query !== 'object' || Array.isArray(query)) return query;
  const source = { ...query };
  if ('page' in source && !('current' in source)) {
    source.current = source.page;
  }
  const reduced = {};
  for (const key of keys) {
    if (key in source) reduced[key] = source[key];
  }
  return reduced;
}

export function stableJson(value) {
  return JSON.stringify(sortForStableJson(value));
}

export function jsonIncludes(value, candidate) {
  return normalizeParityText(JSON.stringify(value)).includes(normalizeParityText(candidate));
}

export function jsonIncludesAny(value, candidates) {
  const json = normalizeParityText(JSON.stringify(value));
  return candidates.some((candidate) => json.includes(normalizeParityText(candidate)));
}

export function requestIncludesParamValue(requests, keyFragment, expectedValue) {
  const expected = String(expectedValue);
  return (requests ?? []).some((request) => {
    const entries = Array.isArray(request?.searchParams) ? request.searchParams : [];
    if (
      entries.some(
        ([key, value]) => String(key).includes(keyFragment) && String(value) === expected,
      )
    ) {
      return true;
    }
    const dataValue = request?.data?.[keyFragment];
    return Array.isArray(dataValue)
      ? dataValue.map(String).includes(expected)
      : String(dataValue ?? '') === expected;
  });
}

export function dashboardSubscribeTargetsMatch(result) {
  const expectedTargets = result.expectedTargets ?? [];
  const itemTexts = result.opened?.itemTexts ?? [];
  const presentTargets = subscribeTargetTitles.filter((target) =>
    itemTexts.some((text) => text.endsWith(target)),
  );
  return (
    result.before?.boxCount === 0 &&
    result.opened?.boxCount >= 1 &&
    Boolean(result.opened?.drawerOpenCount || result.opened?.modalCount) &&
    expectedTargets.every((target) => presentTargets.includes(target)) &&
    presentTargets.every((target) => expectedTargets.includes(target))
  );
}

export function clonePageRequests(requests = []) {
  return (requests ?? []).map((request) =>
    request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
  );
}

export function sortForStableJson(value) {
  if (Array.isArray(value)) {
    return value.map(sortForStableJson);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, sortForStableJson(nested)]),
    );
  }
  return value;
}

export function toPresenceTokens(values, token) {
  return values.length > 0 ? [token] : [];
}
