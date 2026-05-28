import type { GuestConfig } from '@v2board/types';
import type { ApiClient } from '../client';

export const config = (client: ApiClient) =>
  client.request<GuestConfig>({ url: '/guest/comm/config', method: 'GET' });
