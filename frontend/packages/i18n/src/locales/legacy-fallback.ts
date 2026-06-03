export type LegacyDictionary = Record<string, string>;

type TranslationTree =
  | string
  | number
  | boolean
  | null
  | TranslationTree[]
  | { [key: string]: TranslationTree };

export function legacyMessageKey(value: string): string {
  return value.replace(/\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g, '{$1}');
}

export function i18nextMessage(value: string): string {
  return value.replace(
    /\{\{\s*([A-Za-z0-9_]+)\s*\}\}|\{\s*([A-Za-z0-9_]+)\s*\}/g,
    (match, i18nextKey: string | undefined, legacyKey: string | undefined) =>
      i18nextKey ? `{{${i18nextKey}}}` : legacyKey === 'url' ? match : `{{${legacyKey}}}`,
  );
}

export function createLegacySourceReverseMap(
  sourceDict: LegacyDictionary | undefined,
): Map<string, string> | undefined {
  if (!sourceDict) return undefined;
  const reverse = new Map<string, string>();
  for (const [source, translated] of Object.entries(sourceDict)) {
    if (!reverse.has(translated)) reverse.set(translated, source);
    const i18nextTranslated = i18nextMessage(translated);
    if (!reverse.has(i18nextTranslated)) reverse.set(i18nextTranslated, source);
  }
  return reverse;
}

export function translateLegacyDictionary<T extends TranslationTree>(
  tree: T,
  dict: LegacyDictionary | undefined,
  sourceReverse?: Map<string, string>,
): T {
  if (typeof tree === 'string') {
    if (!dict) return tree;
    const legacyKey = legacyMessageKey(tree);
    const sourceKey = sourceReverse?.get(tree) ?? sourceReverse?.get(legacyKey);
    const translated = dict[tree] ?? dict[legacyKey] ?? (sourceKey ? dict[sourceKey] : undefined);
    return (translated === undefined ? tree : i18nextMessage(translated)) as T;
  }
  if (Array.isArray(tree)) {
    return tree.map((item) => translateLegacyDictionary(item, dict, sourceReverse)) as T;
  }
  if (tree && typeof tree === 'object') {
    return Object.fromEntries(
      Object.entries(tree).map(([key, value]) => [
        key,
        translateLegacyDictionary(value, dict, sourceReverse),
      ]),
    ) as T;
  }
  return tree;
}
