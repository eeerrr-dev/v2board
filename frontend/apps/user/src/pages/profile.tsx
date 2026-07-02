import { PageShell } from '@/components/ui/page';
import { AccountCard } from './profile/account-card';
import { GiftCardCard } from './profile/gift-card-card';
import { NotificationsCard } from './profile/notifications-card';
import { PasswordCard } from './profile/password-card';
import { ResetSubscribeCard } from './profile/reset-subscribe-card';
import { SessionsCard } from './profile/sessions-card';
import { TelegramBindCard, TelegramDiscussCard } from './profile/telegram-card';
import { WalletCard } from './profile/wallet-card';

// The account surface is a grid of self-contained shadcn cards. Each card owns
// its own queries, mutations, form, and dialog (see ./profile/*), so this page
// is only the layout. Every Tier-1 contract lives inside a card: the deposit
// Math.round(amount*100) payload (WalletCard), the 0/1 preference toggles
// (WalletCard/NotificationsCard via usePreferenceToggle), the /login redirect
// after a password change (PasswordCard), the never-fetch-on-mount subscribe
// query plus its explicit refetch after a Telegram unbind (TelegramBindCard),
// and the reset-security token rotation (ResetSubscribeCard).
export default function ProfilePage() {
  return (
    <PageShell data-testid="profile-page">
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
        <WalletCard />
        <GiftCardCard />
      </div>

      <AccountCard />

      <SessionsCard />

      <div className="grid gap-6 lg:grid-cols-2">
        <PasswordCard />
        <NotificationsCard />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <TelegramBindCard />
        <TelegramDiscussCard />
        <ResetSubscribeCard className="lg:col-span-2" />
      </div>
    </PageShell>
  );
}
