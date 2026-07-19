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

export function requestErrorMessage(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败';
}
