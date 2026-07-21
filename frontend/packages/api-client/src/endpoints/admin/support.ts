import type { InternalApiOperationMap, Ticket, TicketReplyPayload } from '@v2board/types';
import type { ApiClient } from '../../client';
import { requestInternal } from '../../internal-operation';
import type { PageResult, QueryRequestConfig } from './shared';

type AdminTicketItem = InternalApiOperationMap['adminTicketsList']['response']['items'][number];

function toTicket(ticket: AdminTicketItem): Ticket {
  if (![0, 1, 2].includes(ticket.level)) {
    throw new TypeError(`Unsupported ticket level: ${ticket.level}`);
  }
  if (![0, 1].includes(ticket.status)) {
    throw new TypeError(`Unsupported ticket status: ${ticket.status}`);
  }
  if (![0, 1].includes(ticket.reply_status)) {
    throw new TypeError(`Unsupported ticket reply status: ${ticket.reply_status}`);
  }
  return {
    ...ticket,
    level: ticket.level as Ticket['level'],
    status: ticket.status as Ticket['status'],
    reply_status: ticket.reply_status as Ticket['reply_status'],
  };
}

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
  const page = await requestInternal(client, 'adminTicketsList', {
    query: {
      page: query.current,
      per_page: query.pageSize,
      status: query.status,
      email: query.email || undefined,
      reply_status: query.reply_status?.length ? query.reply_status : undefined,
    },
    ...config,
  });
  return { data: page.items.map(toTicket), total: page.total };
};

/** GET /{secure_path}/tickets/{id} — bare detail with the `message[]` thread (§6.5, W14). */
export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminTicketsGet', {
    path: { id: Number(id) },
    ...config,
  });

/** POST /{secure_path}/tickets/{id}/replies `{message}` — 204; the `id` moves to the path (§6.5). */
export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) => {
  const { id, ...data } = payload;
  if (data.message === undefined) throw new TypeError('Ticket reply message is required');
  return requestInternal(client, 'adminTicketsRepliesCreate', {
    path: { id: Number(id) },
    data: { message: data.message },
  });
};

/** POST /{secure_path}/tickets/{id}/close — 204, no body (§6.5, W14). */
export const closeTicket = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminTicketsClose', {
    path: { id },
  });
