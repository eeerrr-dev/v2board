import { useEffect, useState } from 'react';

export function useLegacyFetchLoading(isFetching: boolean, error?: unknown) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  return mounted && (isFetching || isLegacyTransportError(error));
}

function isLegacyTransportError(error: unknown): boolean {
  return (
    error !== null &&
    typeof error === 'object' &&
    'status' in error &&
    Number((error as { status?: unknown }).status) === 0
  );
}
