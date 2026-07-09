import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';

// Localized empty-table description for the current language. Shared by the
// list pages (traffic/invite/tickets/knowledge) so each one stops re-plumbing
// getLocaleAntdMessages(i18n.language).emptyDescription locally.
export function useEmptyDescription(): string {
  const { i18n } = useTranslation();
  return getLocaleAntdMessages(i18n.language).emptyDescription;
}
