export type TicketStatus = 0 | 1;
export type TicketLevel = 0 | 1 | 2;

/**
 * Ticket thread message — dialect v2 (docs/api-dialect.md §5.7 W8, §6.5
 * W14): RFC 3339 timestamps, boolean caller-relative `is_me`.
 */
export interface TicketMessage {
  id: number;
  user_id: number;
  ticket_id: number;
  message: string;
  is_me: boolean;
  created_at: string;
  updated_at: string;
}

/**
 * Ticket row — dialect v2 (§5.7 W8, §6.5 W14): RFC 3339 timestamps; `level`/
 * `status`/`reply_status` stay numeric enums (§4.1).
 */
export interface Ticket {
  id: number;
  user_id: number;
  subject: string;
  level: TicketLevel;
  status: TicketStatus;
  reply_status: 0 | 1;
  last_reply_user_id?: number | null;
  created_at: string;
  updated_at: string;
  message?: TicketMessage[];
}

export interface TicketCreatePayload {
  subject?: string;
  level?: TicketLevel;
  message?: string;
}

export interface TicketReplyPayload {
  id: number | string;
  message?: string;
}

export interface TicketWithdrawPayload {
  withdraw_method?: string;
  withdraw_account?: string;
}
