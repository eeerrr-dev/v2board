export type TicketStatus = 0 | 1;
export type TicketLevel = 0 | 1 | 2;

export interface TicketMessage {
  id: number;
  user_id: number;
  ticket_id: number;
  message: string;
  is_me: boolean;
  created_at: number;
  updated_at: number;
}

export interface Ticket {
  id: number;
  user_id: number;
  subject: string;
  level: TicketLevel;
  status: TicketStatus;
  reply_status: 0 | 1;
  last_reply_user_id: number | null;
  created_at: number;
  updated_at: number;
  message?: TicketMessage[];
}

export interface TicketCreatePayload {
  subject?: string;
  level?: TicketLevel;
  message?: string;
}

export interface TicketReplyPayload {
  id: number;
  message?: string;
}

export interface TicketWithdrawPayload {
  withdraw_method?: string;
  withdraw_account?: string;
}
