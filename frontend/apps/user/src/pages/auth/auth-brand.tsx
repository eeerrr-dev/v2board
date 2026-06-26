import { getLegacyDescription, getLegacyLogo, getLegacyTitle } from '@/lib/legacy-settings';

export function AuthBrand() {
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
        <h1 className="v2board-auth-title v2board-auth-title--wordmark tw:text-3xl tw:font-semibold tw:tracking-tight">
          {title || 'V2Board'}
        </h1>
      )}
      {description ? (
        <p className="tw:mt-2 tw:text-sm tw:text-foreground-muted">{description}</p>
      ) : null}
    </div>
  );
}
