# @v2board/ui

The canonical source package for UI shared by the user and admin applications.

- `src/components` owns cross-application shadcn/Radix primitives.
- `src/styles` owns the shared token, font, and base-layer CSS.
- `src/lib` and `src/hooks` own design-system utilities used by both shells.
- Tests for shared components, hooks, and utilities live beside this package's sources only.
- App-specific compositions remain in each application's `components/ui` directory.

Import public modules through explicit package subpaths such as
`@v2board/ui/button`; do not recreate forwarding copies inside an application.
