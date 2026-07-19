import { useLocation } from 'react-router';
import { ServerGroupPage } from './group-page';
import { ServerManagePage } from './manage-page';
import { ServerRoutePage } from './route-page';

export {
  applyServerNodeColumnControls,
  createServerSortPayload,
  getBinarySelectValue,
  getNetworkSettingsPlaceholder,
  getNumericSelectValue,
  getV2nodeSecurityValue,
  moveServerNodeByDragIndexes,
} from './domain';

export default function ServersPage() {
  const location = useLocation();
  if (location.pathname === '/server/group') return <ServerGroupPage />;
  if (location.pathname === '/server/route') return <ServerRoutePage />;
  if (location.pathname === '/server/manage') return <ServerManagePage />;

  return null;
}
