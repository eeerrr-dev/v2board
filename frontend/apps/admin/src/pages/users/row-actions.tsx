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
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" data-testid={`user-actions-${row.id}`}>
          操作
          <ChevronDown className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => onAction('edit', row)} data-testid={`user-edit-${row.id}`}>
          <Pencil className="size-4" />
          编辑
        </DropdownMenuItem>
        <DropdownMenuItem disabled={assignDisabled} onClick={() => onAction('assign', row)}>
          <Plus className="size-4" />
          分配订单
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('copy', row)} data-testid={`user-copy-${row.id}`}>
          <Copy className="size-4" />
          复制订阅URL
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('reset', row)}>
          <RefreshCw className="size-4" />
          重置UUID及订阅URL
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('orders', row)}>
          <ReceiptText className="size-4" />
          TA的订单
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('invite', row)}>
          <UsersRound className="size-4" />
          TA的邀请
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('traffic', row)}>
          <Activity className="size-4" />
          TA的流量记录
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onClick={() => onAction('delete', row)}
          data-testid={`user-delete-${row.id}`}
        >
          <Trash2 className="size-4" />
          删除用户
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
