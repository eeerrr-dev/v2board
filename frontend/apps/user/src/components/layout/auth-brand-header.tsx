import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';

// Authored V2Board — shared 2026 auth brand header. The logo/title/description block is identical on
// login, register, and forget; it lives here so the single load-bearing invariant (the operator logo
// wrapped in the page's one <h1>, otherwise the title text as the <h1> carrying the unlayered
// `.v2board-auth-title` color hook) is owned in one place instead of copied three times.
//
// The title color is owned by the authored `.v2board-auth-title` rule in user-auth-surface.css, not a
// tw:text-foreground utility: vendored (unlayered) `h1{color}` heading rules outrank any layered
// Tailwind color utility under CSS cascade layers, so the utility would be inert and the heading would
// stay antd-black (unreadable on the dark card).
export function AuthBrandHeader() {
  const logo = getLegacyLogo();
  const title = getLegacyTitle();
  const description = getLegacyDescription();

  return (
    <div className="tw:mb-7 tw:text-center">
      {logo ? (
        <h1 className="tw:m-0">
          <img className="v2board-logo tw:mx-auto tw:h-11 tw:w-auto" src={logo} alt={title || 'V2Board'} />
        </h1>
      ) : (
        <h1 className="v2board-auth-title tw:text-2xl tw:font-semibold tw:tracking-tight">
          {title || 'V2Board'}
        </h1>
      )}
      {description ? (
        <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
      ) : null}
    </div>
  );
}
