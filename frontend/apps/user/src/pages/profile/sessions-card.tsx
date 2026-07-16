import { useTranslation } from 'react-i18next';
import { MonitorSmartphone } from 'lucide-react';
import { formatBackendDateTime } from '@v2board/config/format';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { ErrorState } from '@/components/ui/error-state';
import { Spinner } from '@/components/ui/spinner';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from '@/components/ui/table';
import { useActiveSessions, useRemoveSessionMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import { SectionIcon } from './profile-ui';

// Native responses identify the current opaque session explicitly. The UI does
// not inspect or compare bearer credentials to infer session identity.
export function SessionsCard() {
  const { t } = useTranslation();
  const sessions = useActiveSessions({ refetchOnMount: 'always' });
  const removeSession = useRemoveSessionMutation();

  const entries = sessions.data
    ? Object.entries(sessions.data).sort(([, a], [, b]) => b.login_at - a.login_at)
    : [];

  const onRevoke = (sessionId: string) => {
    void confirmDialog({
      title: t(($) => $.common.attention),
      description: t(($) => $.profile.session_revoke_confirm),
      confirmText: t(($) => $.profile.session_revoke),
      onConfirm: () =>
        removeSession.mutateAsync(sessionId).then(() => {
          toast.success(t(($) => $.profile.session_revoke_success));
        }),
    });
  };

  return (
    <Card data-testid="profile-sessions-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <MonitorSmartphone className="size-4" />
          </SectionIcon>
          <div className="space-y-1">
            <CardTitle className="text-lg" data-testid="profile-card-title">
              {t(($) => $.profile.active_sessions)}
            </CardTitle>
            <CardDescription>{t(($) => $.profile.active_sessions_desc)}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        {sessions.isLoading ? (
          <div className="flex items-center gap-2 px-6 pb-6 text-sm text-muted-foreground">
            <Spinner className="size-4" />
            <span>{t(($) => $.common.loading)}</span>
          </div>
        ) : sessions.isError ? (
          <div className="px-6 pb-6">
            <ErrorState
              onRetry={() => void sessions.refetch()}
              data-testid="profile-sessions-error"
            />
          </div>
        ) : entries.length === 0 ? (
          <div
            className="px-6 pb-6 text-sm text-muted-foreground"
            data-testid="profile-sessions-empty"
          >
            {t(($) => $.profile.no_sessions)}
          </div>
        ) : (
          <TableScroll className="pb-2">
            <Table className="min-w-[560px]">
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>{t(($) => $.profile.session_device)}</TableHead>
                  <TableHead>{t(($) => $.profile.session_ip)}</TableHead>
                  <TableHead>{t(($) => $.profile.session_login_at)}</TableHead>
                  <TableHead className="text-right">
                    <span className="sr-only">{t(($) => $.profile.session_revoke)}</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map(([sessionId, session]) => {
                  const isCurrent = session.current;
                  return (
                    <TableRow key={sessionId} data-testid="profile-session-row">
                      <TableCell className="max-w-[18rem]">
                        <div className="flex items-center gap-2">
                          <span className="min-w-0 truncate" title={session.ua}>
                            {session.ua || '—'}
                          </span>
                          {isCurrent ? (
                            <Badge variant="secondary" data-testid="profile-session-current">
                              {t(($) => $.profile.session_current)}
                            </Badge>
                          ) : null}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono text-xs">{session.ip || '—'}</TableCell>
                      <TableCell className="whitespace-nowrap text-muted-foreground">
                        {formatBackendDateTime(session.login_at)}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="text-destructive hover:text-destructive"
                          disabled={isCurrent}
                          data-testid="profile-session-revoke"
                          onClick={() => onRevoke(sessionId)}
                        >
                          {t(($) => $.profile.session_revoke)}
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableScroll>
        )}
      </CardContent>
    </Card>
  );
}
