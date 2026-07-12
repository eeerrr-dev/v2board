import { useState } from 'react';
import { useUpdateProfileMutation } from '@/lib/queries';

export type ProfilePreferenceKey = 'auto_renewal' | 'remind_expire' | 'remind_traffic';

// Owns the 0/1 preference toggle both the wallet card (auto_renewal) and the
// notifications card (remind_expire / remind_traffic) run. Each card calls this
// hook so it holds its own mutation instance and per-key pending state — no
// page-level shared state survives the split — while keeping the exact 0/1
// payload contract and leaving the user-record refresh to the mutation's
// onSuccess (Tier-1: the payload; Tier-2: the pending presentation).
export function usePreferenceToggle() {
  const updateProfile = useUpdateProfileMutation();
  const [pending, setPending] = useState<Partial<Record<ProfilePreferenceKey, boolean>>>({});

  const toggle = (key: ProfilePreferenceKey, value: 0 | 1) => {
    setPending((current) => ({ ...current, [key]: true }));
    updateProfile.mutate({ [key]: value } as Parameters<typeof updateProfile.mutate>[0], {
      onSettled: () => {
        setPending((current) => ({ ...current, [key]: false }));
      },
    });
  };

  return { toggle, pending };
}
