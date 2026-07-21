const BLOCKING_IMPACTS = new Set(['critical', 'serious']);
const ACCESSIBILITY_TAGS = [
  'wcag2a',
  'wcag2aa',
  'wcag21a',
  'wcag21aa',
  'wcag22a',
  'wcag22aa',
  'best-practice',
];

// These selectors identify content rendered and owned by external vendors. We
// exclude a root only when it is actually present on the page; all first-party
// markup (including backend-authored `.custom-html-style`) remains in scope.
// This avoids both flaky cross-origin scans and broad, silent rule suppression.
const THIRD_PARTY_ROOTS = [
  {
    owner: 'Google reCAPTCHA',
    selector:
      'iframe[src*="google.com/recaptcha"], iframe[src*="recaptcha.net/recaptcha"]',
  },
];

function formatBlockingViolations(violations) {
  return violations
    .map((violation) => {
      const nodes = violation.nodes
        .slice(0, 5)
        .map((node) => {
          const target = node.target.join(' ');
          const html = node.html.replace(/\s+/g, ' ').slice(0, 240);
          return `${target}: ${html}`;
        })
        .join(' | ');
      return `${violation.impact} ${violation.id}: ${violation.help} (${nodes}) — ${violation.helpUrl}`;
    })
    .join('\n');
}

export async function runAccessibilitySmokeInteraction(page) {
  // Keep the dependency lazy: parity-config-audit imports interaction metadata
  // directly from the read-only source mount, where runtime dependencies are
  // deliberately unavailable. Real Playwright runs execute from the installed
  // Docker workspace and resolve the official integration here.
  const { default: AxeBuilder } = await import('@axe-core/playwright');
  await page.emulateMedia({ reducedMotion: 'reduce' });

  let builder = new AxeBuilder({ page }).withTags(ACCESSIBILITY_TAGS);
  const excludedThirdPartyRoots = [];

  for (const root of THIRD_PARTY_ROOTS) {
    if ((await page.locator(root.selector).count()) === 0) continue;
    builder = builder.exclude(root.selector);
    excludedThirdPartyRoots.push(root.owner);
  }

  const results = await builder.analyze();
  const blockingViolations = results.violations.filter((violation) =>
    BLOCKING_IMPACTS.has(violation.impact),
  );

  if (blockingViolations.length > 0) {
    throw new Error(
      `axe found ${blockingViolations.length} critical/serious violation(s):\n${formatBlockingViolations(blockingViolations)}`,
    );
  }

  return {
    blockingViolationCount: 0,
    excludedThirdPartyRoots,
    scanned: true,
    scannedRuleCount: results.passes.length + results.violations.length,
  };
}
