import { adminAudit } from './audit';
import { adminAuth } from './auth';
import { adminConfig } from './config';
import { adminCoupons } from './coupons';
import { adminDashboard } from './dashboard';
import { adminKnowledge } from './knowledge';
import { adminNav } from './nav';
import { adminNotices } from './notices';
import { adminOrders } from './orders';
import { adminPayments } from './payments';
import { adminPlans } from './plans';
import { adminServers } from './servers';
import { adminShared } from './shared';
import { adminSystem } from './system';
import { adminTickets } from './tickets';
import { adminUsers } from './users';

// English translation of the admin surface, mirroring the shape of
// ../admin/index.ts's adminZh tree exactly (see that file's comment for the
// per-locale wiring convention).
export const adminEn = {
  audit: adminAudit,
  auth: adminAuth,
  config: adminConfig,
  coupons: adminCoupons,
  dashboard: adminDashboard,
  knowledge: adminKnowledge,
  nav: adminNav,
  notices: adminNotices,
  orders: adminOrders,
  payments: adminPayments,
  plans: adminPlans,
  servers: adminServers,
  shared: adminShared,
  system: adminSystem,
  tickets: adminTickets,
  users: adminUsers,
};
