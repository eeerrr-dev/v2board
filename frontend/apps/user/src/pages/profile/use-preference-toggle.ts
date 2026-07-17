import { useState } from 'react';
import { useUpdateProfileMutation } from '@/lib/queries';

export type ProfilePreferenceKey = 'auto_renewal' | 'remind_expire' | 'remind_traffic';

// Owns the boolean preference toggle both the wallet card (auto_renewal) and
// the notifications card (remind_expire / remind_traffic) run. Each card calls
// this hook so it holds its own mutation instance and per-key pending state —
// no page-level shared state survives the split — while sending only the one
// changed flag (PATCH /user/profile, §4.4 absent-retains) and leaving the
// user-record refresh to the mutation's onSuccess (Tier-1: the payload;
// Tier-2: the pending presentation).
export function usePreferenceToggle() {
  const updateProfile = useUpdateProfileMutation();
  const [pending, setPending] = useState<Partial<Record<ProfilePreferenceKey, boolean>>>({});

  const toggle = (key: ProfilePreferenceKey, value: boolean) => {
    setPending((current) => ({ ...current, [key]: true }));
    updateProfile.mutate({ [key]: value } as Parameters<typeof updateProfile.mutate>[0], {
      onSettled: () => {
        setPending((current) => ({ ...current, [key]: false }));
      },
    });
  };

  return { toggle, pending };
}
