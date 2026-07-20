import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, RefreshCcw } from 'lucide-react';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { useResetSubscribeMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import { ProfileConfirmDialog, SectionIcon } from './profile-ui';

export function ResetSubscribeCard({ className }: { className?: string }) {
  const { t } = useTranslation();
  const resetSub = useResetSubscribeMutation();
  const [confirmOpen, setConfirmOpen] = useState(false);

  const onConfirmReset = () => {
    setConfirmOpen(false);
    // The subscribe-token rotation (POST /user/subscription/reset-token) is
    // the Tier-1 outcome here; the shared mutation error presenter owns
    // failures.
    resetSub.mutate(undefined, {
      onSuccess: () => {
        toast.success(t(($) => $.profile.reset_success));
      },
    });
  };

  return (
    <Card className={className} data-testid="profile-reset-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <RefreshCcw className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t(($) => $.profile.reset_subscribe)}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <Alert data-testid="profile-reset-warning">
          <AlertCircle className="size-4" />
          <AlertDescription>{t(($) => $.profile.reset_subscribe_warning)}</AlertDescription>
        </Alert>
        <Button
          className="w-full sm:w-fit"
          data-testid="profile-reset-button"
          variant="destructive"
          onClick={() => setConfirmOpen(true)}
        >
          {t(($) => $.profile.reset)}
        </Button>
      </CardContent>

      <ProfileConfirmDialog
        open={confirmOpen}
        title={t(($) => $.profile.reset_subscribe_confirm)}
        description={t(($) => $.profile.reset_subscribe_tip)}
        onCancel={() => setConfirmOpen(false)}
        onConfirm={onConfirmReset}
      />
    </Card>
  );
}
