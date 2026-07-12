import { useTranslation } from 'react-i18next';
import { Bell } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useUserInfo } from '@/lib/queries';
import { ProfileSwitch, SectionIcon } from './profile-ui';
import { usePreferenceToggle } from './use-preference-toggle';

export function NotificationsCard() {
  const { t } = useTranslation();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const { toggle, pending } = usePreferenceToggle();
  const data = info.data;

  return (
    <Card data-testid="profile-notifications-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <Bell className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t($ => $.profile.notifications)}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <PreferenceRow
          label={t($ => $.profile.remind_expire)}
          checked={data?.remind_expire}
          loading={pending.remind_expire}
          onChange={(checked) => void toggle('remind_expire', checked ? 1 : 0)}
        />
        <PreferenceRow
          label={t($ => $.profile.remind_traffic)}
          checked={data?.remind_traffic}
          loading={pending.remind_traffic}
          onChange={(checked) => void toggle('remind_traffic', checked ? 1 : 0)}
        />
      </CardContent>
    </Card>
  );
}

function PreferenceRow({
  label,
  checked,
  loading,
  onChange,
}: {
  label: string;
  checked?: unknown;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border p-4">
      <div className="text-sm font-medium leading-5">{label}</div>
      <ProfileSwitch ariaLabel={label} checked={checked} loading={loading} onChange={onChange} />
    </div>
  );
}
