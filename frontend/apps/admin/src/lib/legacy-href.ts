// React 19 blocks `javascript:` href props by rewriting them to a throwing stub.
// The packaged admin theme ran on React 16 and rendered literal
// `javascript:void(0);` anchors. Set the attribute through a ref so the DOM stays
// byte-compatible with the old bundle without React rewriting it.
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
