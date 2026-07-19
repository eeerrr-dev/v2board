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

/**
 * Admin-surface copy, one subtree per page family. A single shared tree feeds
 * every locale file: admin copy is deliberately untranslated (Chinese) until
 * product translations are supplied (AGENTS.md, Admin Surface Direction). To
 * translate a locale, replace this import in that locale file with a fully
 * translated tree of the same shape.
 */
export const adminZh = {
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
