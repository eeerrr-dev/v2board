import { useLocation } from 'react-router';
import { ServerGroupPage } from './server-group-page';
import { ServerManagePage } from './server-manage-page';
import { ServerRoutePage } from './server-route-page';

export {
  applyServerNodeColumnControls,
  createServerSortPayload,
  getBinarySelectValue,
  getNetworkSettingsPlaceholder,
  getNumericSelectValue,
  getV2nodeSecurityValue,
  moveServerNodeByDragIndexes,
} from './server-domain';

export default function ServersPage() {
  const location = useLocation();
  if (location.pathname === '/server/group') return <ServerGroupPage />;
  if (location.pathname === '/server/route') return <ServerRoutePage />;
  if (location.pathname === '/server/manage') return <ServerManagePage />;

  return null;
}
