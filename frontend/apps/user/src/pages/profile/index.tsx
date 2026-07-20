import { PageShell } from '@v2board/ui/page';
import { AccountCard } from './account-card';
import { GiftCardCard } from './gift-card-card';
import { NotificationsCard } from './notifications-card';
import { PasswordCard } from './password-card';
import { ResetSubscribeCard } from './reset-subscribe-card';
import { SessionsCard } from './sessions-card';
import { TelegramBindCard, TelegramDiscussCard } from './telegram-card';
import { WalletCard } from './wallet-card';

// The account surface is a grid of self-contained shadcn cards. Each card owns
// its own queries, mutations, form, and dialog (see the sibling ./*-card files), so this page
// is only the layout. Every Tier-1 contract lives inside a card: the deposit
// exact major-unit-to-cents deposit boundary (WalletCard/api-client), the 0/1 preference toggles
// (WalletCard/NotificationsCard via usePreferenceToggle), the /login redirect
// after a password change (PasswordCard), the never-fetch-on-mount subscribe
// query plus its explicit refetch after a Telegram unbind (TelegramBindCard),
// and the reset-security token rotation (ResetSubscribeCard).
export default function ProfilePage() {
  return (
    <PageShell data-testid="profile-page">
      <div className="grid gap-6 @4xl/main:grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
        <WalletCard />
        <GiftCardCard />
      </div>

      <AccountCard />

      <SessionsCard />

      <div className="grid gap-6 @3xl/main:grid-cols-2">
        <PasswordCard />
        <NotificationsCard />
      </div>

      <div className="grid gap-6 @3xl/main:grid-cols-2">
        <TelegramBindCard />
        <TelegramDiscussCard />
        <ResetSubscribeCard className="@3xl/main:col-span-2" />
      </div>
    </PageShell>
  );
}
