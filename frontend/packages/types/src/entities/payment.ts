export interface PaymentMethod {
  id: number;
  name: string;
  payment: string;
  icon: string | null;
  handling_fee_fixed: number | null;
  handling_fee_percent: number | null;
}

/**
 * Admin payment row (docs/api-dialect.md §6.2, W11): boolean `enable`, RFC 3339
 * timestamps, server-redacted `config`. `legacy_md5_signature`/`security_warning`
 * flag MD5-signature providers.
 */
export interface AdminPayment extends PaymentMethod {
  uuid: string;
  config: Record<string, string>;
  notify_domain: string | null;
  notify_url: string;
  enable: boolean;
  sort: number | null;
  created_at: string;
  updated_at: string;
  legacy_md5_signature?: boolean;
  security_warning?: string | null;
}

export interface PaymentFormField {
  label: string;
  description?: string;
  type?: string;
  value?: string;
}

export type PaymentFormDefinition = Record<string, PaymentFormField>;
