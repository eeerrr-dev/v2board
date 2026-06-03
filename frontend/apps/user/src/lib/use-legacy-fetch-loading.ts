import { useEffect, useState } from 'react';

export function useLegacyFetchLoading(isFetching: boolean) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  return mounted && isFetching;
}
