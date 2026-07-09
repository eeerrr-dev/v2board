// Single source of truth for the shared dialog presentation surface. The two
// Radix roots — @radix-ui/react-dialog (Dialog, Sheet) and
// @radix-ui/react-alert-dialog (AlertDialog) — render identical overlay/content
// geometry and header/footer/title/description styling, so the class strings
// live here once and cannot drift apart.
//
// The content centering depends on the `-translate-*-1/2` utilities, which
// Tailwind v4 compiles to the independent `translate:` property; the open/close
// keyframes in styles/user-shadcn-motion.css must never re-apply
// `transform: translate(-50%, ...)` on top of them. See
// styles/dialog-centering.test.ts for the regression guard.

export const dialogOverlayClassName = 'v2board-radix-overlay fixed inset-0 z-50 bg-black/50';

export const dialogContentClassName =
  'v2board-island v2board-radix-dialog-content fixed top-1/2 left-1/2 z-50 grid w-full max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-lg border border-border bg-background p-6 text-foreground shadow-lg outline-none sm:max-w-lg';

export const dialogHeaderClassName = 'flex flex-col gap-2 text-center sm:text-left';

export const dialogFooterClassName = 'flex flex-col-reverse gap-2 sm:flex-row sm:justify-end';

export const dialogTitleClassName = 'text-lg font-semibold leading-none text-foreground';

export const dialogDescriptionClassName = 'text-sm text-muted-foreground';
