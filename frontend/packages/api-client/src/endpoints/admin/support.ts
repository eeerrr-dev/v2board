import type { Ticket, TicketReplyPayload } from '@v2board/types';
import type { ApiClient } from '../../client';
import { pageSchema } from '../../dialect';
import { noContentSchema, userTicketDetailSchema, userTicketSchema } from '../../contracts';
import type { PageResult, QueryRequestConfig } from './shared';

/**
 * §6.5 (W14) list query: pages keep their local `{current, pageSize}` state;
 * the §8 `page`/`per_page` wire query is minted here. `status`, `email`, and
 * the repeatable `reply_status` keys are the admin ticket list's only
 * filters (no §7 DSL — the spec invents none for this family).
 */
export interface AdminTicketListQuery {
  current?: number;
  pageSize?: number;
  status?: number;
  email?: string;
  reply_status?: number[] | null;
}

/**
 * GET /{secure_path}/tickets — dialect v2 `{items, total}` page (§6.5, W14).
 * `reply_status` rides as a repeated real-array query key (the legacy
 * JSON-stringified array param died); an empty `email` means "no filter",
 * matching the legacy falsy guard, so it is omitted from the wire.
 */
export const fetchTickets = async (
  client: ApiClient,
  query: AdminTicketListQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<Ticket>> => {
  const page = await client.request({
    url: client.resolveAdminPath('/tickets'),
    method: 'GET',
    dialect: 'v2',
    params: {
      page: query.current,
      per_page: query.pageSize,
      status: query.status,
      email: query.email || undefined,
      reply_status: query.reply_status?.length ? query.reply_status : undefined,
    },
    responseSchema: pageSchema(userTicketSchema),
    ...config,
  });
  return { data: page.items, total: page.total };
};

/** GET /{secure_path}/tickets/{id} — bare detail with the `message[]` thread (§6.5, W14). */
export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}`),
    method: 'GET',
    dialect: 'v2',
    responseSchema: userTicketDetailSchema,
    ...config,
  });

/** POST /{secure_path}/tickets/{id}/replies `{message}` — 204; the `id` moves to the path (§6.5). */
export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) => {
  const { id, ...data } = payload;
  return client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}/replies`),
    method: 'POST',
    dialect: 'v2',
    data,
    responseSchema: noContentSchema,
  });
};

/** POST /{secure_path}/tickets/{id}/close — 204, no body (§6.5, W14). */
export const closeTicket = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}/close`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });
