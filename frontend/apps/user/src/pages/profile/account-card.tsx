import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { CircleUser, Copy } from 'lucide-react';
import { formatLegacyDateTime } from '@v2board/config/format';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useUserInfo } from '@/lib/queries';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';
import { SectionIcon } from './profile-ui';

export function AccountCard() {
  const { t } = useTranslation();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const data = info.data;

  const copyUuid = async () => {
    if (data?.uuid && (await copyText(data.uuid))) toast.success(t('dashboard.copy_success'));
  };

  return (
    <Card data-testid="profile-account-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <CircleUser className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t('profile.account')}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <div className="grid gap-x-6 gap-y-4 sm:grid-cols-2">
          <IdentityField
            label={t('profile.email')}
            value={data?.email}
            testId="profile-account-email"
          />
          <IdentityField
            label={t('profile.uuid')}
            value={data?.uuid}
            testId="profile-account-uuid"
            copyLabel={data?.uuid ? t('common.copy') : undefined}
            onCopy={data?.uuid ? copyUuid : undefined}
          />
          <IdentityField
            label={t('profile.created_at')}
            value={data?.created_at ? formatLegacyDateTime(data.created_at) : undefined}
            testId="profile-account-created"
          />
          <IdentityField
            label={t('profile.last_login')}
            value={data?.last_login_at ? formatLegacyDateTime(data.last_login_at) : undefined}
            testId="profile-account-last-login"
          />
        </div>
      </CardContent>
    </Card>
  );
}

function IdentityField({
  label,
  value,
  testId,
  onCopy,
  copyLabel,
}: {
  label: string;
  value: ReactNode;
  testId: string;
  onCopy?: () => void;
  copyLabel?: string;
}) {
  return (
    <div className="space-y-1">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="flex items-center gap-2 text-sm font-medium text-foreground">
        <span className="min-w-0 truncate" data-testid={testId}>
          {value == null || value === '' ? '—' : value}
        </span>
        {onCopy ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-6 shrink-0 text-muted-foreground"
            aria-label={copyLabel}
            data-testid={`${testId}-copy`}
            onClick={() => void onCopy()}
          >
            <Copy className="size-3.5" />
          </Button>
        ) : null}
      </div>
    </div>
  );
}
