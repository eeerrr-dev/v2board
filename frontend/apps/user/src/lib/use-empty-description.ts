import { useTranslation } from 'react-i18next';

// Localized empty-table description for the current language. Shared by the
// list pages so empty states use the same canonical common message as the rest
// of the application.
export function useEmptyDescription(): string {
  const { t } = useTranslation();
  return t(($) => $.common.empty);
}
