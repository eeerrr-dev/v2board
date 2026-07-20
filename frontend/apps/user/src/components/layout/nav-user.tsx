import { ChevronsUpDown, Languages, LogOut, UserRound } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { signOut } from '@/lib/api';
import { LanguageMenuItems } from './language-menu';
import { Avatar, AvatarFallback } from '@v2board/ui/avatar';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@v2board/ui/dropdown-menu';
import { SidebarMenu, SidebarMenuButton, SidebarMenuItem, useSidebar } from '@v2board/ui/sidebar';

interface NavUserProps {
  email: string;
}

// Sidebar-footer account menu (shadcn dashboard-01): a size-lg SidebarMenuButton
// chip that collapses to the avatar in the icon rail, opening a trigger-width
// dropdown above the card on desktop / below it on mobile. It carries the
// account-scoped actions — profile, the Language submenu (the shell's only
// language switcher), and logout. Navigating also closes the mobile sheet.
// Owns its SidebarMenu/SidebarMenuItem shell so the footer composes as just
// <SidebarFooter><NavUser /></SidebarFooter>.
export function NavUser({ email }: NavUserProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { isMobile, state, setOpenMobile } = useSidebar();
  const initials = getInitials(email);
  const accountLabel = getAccountLabel(email);

  const goto = (to: string) => {
    void navigate(to);
    setOpenMobile(false);
  };

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu modal={false}>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size="lg"
              data-testid="app-avatar-trigger"
              aria-label={email}
              title={email}
              className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
            >
              <Avatar className="size-8 rounded-lg grayscale">
                {/* bg-sidebar-accent, not the bg-muted default: light --muted
                    equals the rail token, which would dissolve the chip into
                    the sidebar; the hairline keeps it defined on the white
                    open-state button too. */}
                <AvatarFallback className="rounded-lg border border-sidebar-border bg-sidebar-accent text-xs font-medium text-muted-foreground">
                  {initials}
                </AvatarFallback>
              </Avatar>
              <span className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-medium">{accountLabel}</span>
                {/* sidebar-foreground/70, not muted-foreground: the muted token
                    misses AA contrast on the sidebar rail background. */}
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
            data-testid="app-avatar-menu"
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
            <DropdownMenuGroup>
              <DropdownMenuItem onSelect={() => goto('/profile')}>
                <UserRound className="size-4" />
                {t(($) => $.nav.profile)}
              </DropdownMenuItem>
              <DropdownMenuSub>
                <DropdownMenuSubTrigger data-testid="app-language-trigger">
                  <Languages className="size-4" />
                  {t(($) => $.common.language)}
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent data-testid="app-language-menu" className="min-w-40">
                  <LanguageMenuItems activeIndicator itemClassName="whitespace-nowrap" />
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onSelect={() => {
                signOut();
                void navigate('/login');
              }}
            >
              <LogOut className="size-4" />
              {t(($) => $.common.logout)}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

function getInitials(email: string): string {
  const local = (email.split('@')[0] ?? '').trim();
  const parts = local.split(/[._\-+]+/).filter(Boolean);
  const letters =
    parts.length >= 2 ? `${parts[0]?.[0] ?? ''}${parts[1]?.[0] ?? ''}` : local.slice(0, 2);
  return letters.toUpperCase() || 'U';
}

function getAccountLabel(email: string): string {
  return (email.split('@')[0] ?? '').trim() || email;
}
