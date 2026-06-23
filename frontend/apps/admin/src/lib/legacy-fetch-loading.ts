export function legacyFetchLoading(isFetching: boolean, error?: unknown): boolean {
  return isFetching || isLegacyTransportError(error);
}

function isLegacyTransportError(error: unknown): boolean {
  return (
    error !== null &&
    typeof error === 'object' &&
    'status' in error &&
    Number((error as { status?: unknown }).status) === 0
  );
}
