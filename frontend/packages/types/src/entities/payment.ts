export interface PaymentMethod {
  id: number;
  name: string;
  payment: string;
  icon: string | null;
  handling_fee_fixed: number | null;
  handling_fee_percent: number | null;
}

export interface AdminPayment extends PaymentMethod {
  uuid: string;
  config: Record<string, string>;
  notify_domain: string | null;
  notify_url: string;
  enable: 0 | 1;
  sort: number | null;
  created_at: number;
  updated_at: number;
}

export interface PaymentFormField {
  label: string;
  description?: string;
}

export type PaymentFormDefinition = Record<string, PaymentFormField>;
