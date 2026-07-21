// The public family — dialect v2 (docs/api-dialect.md §5.1, Appendix A §W3):
// bare success bodies against the `/public/*` routes. The module keeps its
// historical `guest` name so call sites stay stable while the wire moved off
// `/guest/comm/config`.
import type { ApiClient, ApiRequestConfig } from '../client';
import { requestInternal } from '../internal-operation';

export const config = (client: ApiClient, request?: Pick<ApiRequestConfig, 'signal'>) =>
  requestInternal(client, 'publicConfig', {
    ...request,
  });
