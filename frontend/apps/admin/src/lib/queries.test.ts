import { describe, expect, it } from 'vitest';
import { adminKeys, adminQueryOptions } from './queries';
import { adminSessionKeys, adminSessionQueryOptions } from './session-queries';

describe('admin query key identity', () => {
  it('shares canonical session and shell-user query keys with route loaders', () => {
    expect(adminSessionQueryOptions.session().queryKey).toEqual(adminSessionKeys.session);
    expect(adminSessionQueryOptions.userInfo().queryKey).toEqual(adminSessionKeys.userInfo);
  });

  it('keeps disabled query inputs as undefined instead of synthetic ids', () => {
    expect(adminKeys.order(undefined)).toEqual(['admin', 'order', undefined]);
    expect(adminKeys.user(undefined)).toEqual(['admin', 'user', undefined]);
    expect(adminKeys.ticket(undefined)).toEqual(['admin', 'ticket', undefined]);
    expect(adminKeys.statUserTraffic(undefined, {})).toEqual([
      'admin',
      'stat',
      'userTraffic',
      undefined,
      {},
    ]);
    expect(adminKeys.paymentForm(undefined, undefined)).toEqual([
      'admin',
      'payment',
      'form',
      undefined,
      undefined,
    ]);

    expect(adminQueryOptions.order(undefined).queryKey).toEqual(adminKeys.order(undefined));
    expect(adminQueryOptions.user(undefined).queryKey).toEqual(adminKeys.user(undefined));
    expect(adminQueryOptions.ticket(undefined).queryKey).toEqual(adminKeys.ticket(undefined));
    expect(adminQueryOptions.userTraffic(undefined, {}).queryKey).toEqual(
      adminKeys.statUserTraffic(undefined, {}),
    );
    expect(adminQueryOptions.paymentForm(undefined).queryKey).toEqual(
      adminKeys.paymentForm(undefined),
    );
  });
});
