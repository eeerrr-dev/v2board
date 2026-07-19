import { useState } from 'react';
import { ChevronsUpDown, LogOut, ShieldCheck } from 'lucide-react';
import { useNavigate } from 'react-router';
import { signOut } from '@/lib/api';
import { MfaDialog } from '@/components/mfa-dialog';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from '@/components/ui/sidebar';

interface AdminNavUserProps {
  email: string;
}

// Sidebar-footer account menu (shadcn dashboard-01): the size-lg chip collapses
// to the avatar in the icon rail and opens a trigger-width dropdown carrying the
// admin account actions — two-factor settings and logout. Mirrors the user
// shell's NavUser without the profile/language items the admin console does
// not have.
export function AdminNavUser({ email }: AdminNavUserProps) {
  const navigate = useNavigate();
  const { isMobile, state, setOpenMobile } = useSidebar();
  const [mfaOpen, setMfaOpen] = useState(false);
  const initials = getInitials(email);
  const accountLabel = getAccountLabel(email);

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu modal={false}>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size="lg"
              data-testid="admin-avatar-trigger"
              aria-label={email}
              title={email}
              className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
            >
              <Avatar className="size-8 rounded-lg grayscale">
                <AvatarFallback className="rounded-lg border border-sidebar-border bg-sidebar-accent text-xs font-medium text-muted-foreground">
                  {initials}
                </AvatarFallback>
              </Avatar>
              <span className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-medium">{accountLabel}</span>
                <span className="truncate text-xs text-sidebar-foreground/70">{email}</span>
              </span>
              <ChevronsUpDown className="ml-auto size-4 text-muted-foreground" />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            side={isMobile ? 'bottom' : state === 'collapsed' ? 'right' : 'top'}
            sideOffset={4}
            className="w-[var(--radix-dropdown-menu-trigger-width)] min-w-56 rounded-lg"
            data-testid="admin-avatar-menu"
          >
            <DropdownMenuLabel className="p-0 font-normal">
              <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                <Avatar className="size-8 rounded-lg">
                  <AvatarFallback className="rounded-lg text-xs font-medium text-muted-foreground">
                    {initials}
                  </AvatarFallback>
                </Avatar>
                <span className="grid min-w-0 flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-medium">{accountLabel}</span>
                  <span className="truncate text-xs text-muted-foreground">{email}</span>
                </span>
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              data-testid="admin-mfa-settings"
              onSelect={() => {
                setOpenMobile(false);
                setMfaOpen(true);
              }}
            >
              <ShieldCheck className="size-4" />
              两步验证
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              data-testid="admin-logout"
              onSelect={() => {
                setOpenMobile(false);
                signOut();
                void navigate('/login');
              }}
            >
              <LogOut className="size-4" />
              登出
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <MfaDialog open={mfaOpen} onOpenChange={setMfaOpen} />
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

function getInitials(email: string): string {
  const local = (email.split('@')[0] ?? '').trim();
  const parts = local.split(/[._\-+]+/).filter(Boolean);
  const letters =
    parts.length >= 2 ? `${parts[0]?.[0] ?? ''}${parts[1]?.[0] ?? ''}` : local.slice(0, 2);
  return letters.toUpperCase() || 'A';
}

function getAccountLabel(email: string): string {
  return (email.split('@')[0] ?? '').trim() || email;
}
