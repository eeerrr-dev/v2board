// Assign every interaction to exactly one spec group. The grouping mirrors the
// runner-module structure (each run fn is exported by exactly one runner module,
// verified during extraction), so membership is unambiguous and total: an
// interaction's group is whichever runner module owns its `run` function.
import * as auth from './runners/auth.mjs';
import * as dashboard from './runners/dashboard.mjs';
import * as profile from './runners/profile.mjs';
import * as commerce from './runners/commerce.mjs';
import * as service from './runners/service.mjs';
import * as invite from './runners/invite.mjs';
import * as ticket from './runners/ticket.mjs';
import * as knowledge from './runners/knowledge.mjs';
import * as fetchFailure from './runners/fetch-failure.mjs';
import * as adminConfig from './runners/admin/config.mjs';
import * as adminPlan from './runners/admin/plan.mjs';
import * as adminServer from './runners/admin/server.mjs';
import * as adminPayment from './runners/admin/payment.mjs';
import * as adminOrder from './runners/admin/order.mjs';
import * as adminCgnk from './runners/admin/coupon-giftcard-notice-knowledge.mjs';
import * as adminTicket from './runners/admin/ticket.mjs';
import * as adminUser from './runners/admin/user.mjs';
import { interactions } from './interaction-scenarios.mjs';

function runnerSet(module) {
  return new Set(Object.values(module).filter((value) => typeof value === 'function'));
}

const GROUPS = {
  auth: runnerSet(auth),
  dashboard: runnerSet(dashboard),
  profile: runnerSet(profile),
  commerce: runnerSet(commerce),
  service: runnerSet(service),
  invite: runnerSet(invite),
  ticket: runnerSet(ticket),
  knowledge: runnerSet(knowledge),
  'fetch-failure': runnerSet(fetchFailure),
  'admin-config': runnerSet(adminConfig),
  'admin-plan': runnerSet(adminPlan),
  'admin-server': runnerSet(adminServer),
  'admin-payment': runnerSet(adminPayment),
  'admin-order': runnerSet(adminOrder),
  'admin-coupon-giftcard-notice-knowledge': runnerSet(adminCgnk),
  'admin-ticket': runnerSet(adminTicket),
  'admin-user': runnerSet(adminUser),
};

export const GROUP_NAMES = Object.keys(GROUPS);

// Direct match: the interaction's run fn is a top-level export of a runner module.
function groupByRunIdentity(interaction) {
  for (const [name, set] of Object.entries(GROUPS)) {
    if (set.has(interaction.run)) return name;
  }
  return null;
}

// A handful of interactions build their run via a factory (e.g. the subscribe-
// import UA matrix through runDashboardSubscribeImportLinksInteractionFor), so
// their run is a closure, not a module export. Resolve those by inheriting the
// group of a sibling interaction that shares the same scenarioLabel and did map
// directly — keeping the grouping principled instead of hard-coding labels.
let resolvedGroups;

function resolveGroups() {
  if (resolvedGroups) return resolvedGroups;
  resolvedGroups = new Map();
  const groupByScenario = new Map();
  for (const interaction of interactions) {
    const group = groupByRunIdentity(interaction);
    if (!group) continue;
    resolvedGroups.set(interaction, group);
    if (interaction.scenarioLabel && !groupByScenario.has(interaction.scenarioLabel)) {
      groupByScenario.set(interaction.scenarioLabel, group);
    }
  }
  for (const interaction of interactions) {
    if (resolvedGroups.has(interaction)) continue;
    resolvedGroups.set(interaction, groupByScenario.get(interaction.scenarioLabel) ?? null);
  }
  return resolvedGroups;
}

export function groupOf(interaction) {
  return resolveGroups().get(interaction) ?? null;
}
