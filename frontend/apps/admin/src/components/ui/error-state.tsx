import { useTranslation } from 'react-i18next';
import { TriangleAlert } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';

export interface ErrorStateProps {
  /** Invoked by the retry action; omit to hide the retry button. */
  onRetry?: () => void;
  /** Overrides the default `common.error_title` copy. */
  message?: string;
  'data-testid'?: string;
}

/**
 * Shared fetch-failure state so a failed query is never misrepresented as an
 * empty/"go subscribe"/permanent-loading state. Distinct from route-error
 * boundaries (render errors) — this is for data queries that resolve to an
 * error and can simply be refetched.
 */
export function ErrorState({ onRetry, message, 'data-testid': testId }: ErrorStateProps) {
  const { t } = useTranslation();
  return (
    <Alert variant="destructive" data-testid={testId ?? 'error-state'}>
      <TriangleAlert className="size-4" />
      <AlertDescription>
        <span className="flex flex-wrap items-center gap-2">
          <span>{message ?? t('common.error_title')}</span>
          {onRetry ? (
            <Button
              variant="link"
              className="h-auto p-0 text-sm"
              onClick={onRetry}
              data-testid="error-state-retry"
            >
              {t('common.retry')}
            </Button>
          ) : null}
        </span>
      </AlertDescription>
    </Alert>
  );
}
