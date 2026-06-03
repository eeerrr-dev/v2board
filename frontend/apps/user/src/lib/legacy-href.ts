// React 19 blocks `javascript:` URLs passed to the `href` prop: it rewrites them
// to a throwing stub (`javascript:throw new Error('React has blocked a javascript:
// URL …')`), which both diverges from the original DOM and throws an uncaught error
// every time such a link is clicked. The original theme (React 16) renders a literal
// `javascript:void(0);` on every button-like <a>. We restore that exact attribute
// imperatively through a ref — React does not intercept `setAttribute`, so it never
// sees (or rewrites) the javascript: URL. Refs are cached per href string so they
// stay stable across re-renders and only fire on mount.
const refs = new Map<string, (node: HTMLAnchorElement | null) => void>();

export function legacyHref(href = 'javascript:void(0);') {
  let ref = refs.get(href);
  if (!ref) {
    ref = (node: HTMLAnchorElement | null) => {
      if (node) node.setAttribute('href', href);
    };
    refs.set(href, ref);
  }
  return ref;
}
