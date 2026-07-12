// Single source of truth for the shared dialog presentation surface. The two
// Radix roots — radix-ui's Dialog (Dialog, Sheet) and AlertDialog namespaces —
// render identical overlay/content
// geometry and header/footer/title/description styling, so the class strings
// live here once and cannot drift apart.
//
// The content centering depends on Tailwind v4's independent `translate:`
// property. tw-animate-css owns only the keyframe `transform`, so zoom motion
// composes with centering instead of applying a second -50% translation.

export const dialogOverlayClassName =
  'fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0 data-[state=closed]:duration-150 data-[state=open]:duration-200 motion-reduce:animate-none!';

export const dialogContentClassName =
  'fixed top-1/2 left-1/2 z-50 grid max-h-[calc(100svh-2rem)] w-full max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 gap-4 overflow-y-auto overscroll-contain scroll-py-6 rounded-lg border border-border bg-background p-6 text-foreground shadow-lg outline-none data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0 data-[state=open]:zoom-in-95 data-[state=closed]:zoom-out-95 data-[state=closed]:duration-150 data-[state=open]:duration-200 motion-reduce:animate-none! sm:max-w-lg';

export const dialogHeaderClassName = 'flex flex-col gap-2 text-center sm:text-left';

export const dialogFooterClassName = 'flex flex-col-reverse gap-2 sm:flex-row sm:justify-end';

export const dialogTitleClassName = 'text-lg font-semibold leading-none text-foreground';

export const dialogDescriptionClassName = 'text-sm text-muted-foreground';
