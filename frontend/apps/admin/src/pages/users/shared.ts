export interface PlanOption {
  label: string;
  value: number;
}

export interface FilterField {
  key: string;
  title: string;
  condition: string[];
  type?: 'text' | 'select' | 'date';
  options?: { label: string; value: string | number }[];
}

export const PLAN_NONE = 'null';

// Returns either the raw error message or the 'admin.users.request_failed' i18n
// key; FieldError resolves it through translateRuntimeMessage at display time.
export function requestErrorMessage(error: unknown) {
  return error instanceof Error && error.message ? error.message : 'admin.users.request_failed';
}
