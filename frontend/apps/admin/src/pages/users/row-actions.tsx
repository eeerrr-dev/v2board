import {
  Activity,
  ChevronDown,
  Copy,
  Pencil,
  Plus,
  ReceiptText,
  RefreshCw,
  Trash2,
  UsersRound,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { AdminUserRow } from '@v2board/types';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

export function UserRowActions({
  row,
  onAction,
  assignDisabled,
}: {
  row: AdminUserRow;
  onAction: (key: string, row: AdminUserRow) => void;
  assignDisabled: boolean;
}) {
  const { t } = useTranslation();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" data-testid={`user-actions-${row.id}`}>
          {t(($) => $.common.operation)}
          <ChevronDown className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => onAction('edit', row)} data-testid={`user-edit-${row.id}`}>
          <Pencil className="size-4" />
          {t(($) => $.common.edit)}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={assignDisabled} onClick={() => onAction('assign', row)}>
          <Plus className="size-4" />
          {t(($) => $.admin.users.assign_order)}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('copy', row)} data-testid={`user-copy-${row.id}`}>
          <Copy className="size-4" />
          {t(($) => $.admin.users.copy_subscribe_url)}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('reset', row)}>
          <RefreshCw className="size-4" />
          {t(($) => $.admin.users.reset_uuid)}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('orders', row)}>
          <ReceiptText className="size-4" />
          {t(($) => $.admin.users.their_orders)}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('invite', row)}>
          <UsersRound className="size-4" />
          {t(($) => $.admin.users.their_invites)}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('traffic', row)}>
          <Activity className="size-4" />
          {t(($) => $.admin.users.their_traffic)}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onClick={() => onAction('delete', row)}
          data-testid={`user-delete-${row.id}`}
        >
          <Trash2 className="size-4" />
          {t(($) => $.admin.users.delete_user)}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
