import { normalizeParityText } from './text.mjs';
import {
  adminFormFieldSelector,
  adminFormLabelSelector,
  adminOverlayOpenSelector,
  adminSelectDropdownSelector,
  adminSelectOptionSelector,
  adminSelectTriggerSelector,
} from './selectors.mjs';
import { delay } from './api-fixtures.mjs';

export async function visibleTexts(page, selector, limit = 10) {
  return page.evaluate(
    ({ limit: maxItems, selector: targetSelector }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      return Array.from(document.querySelectorAll(targetSelector))
        .filter(isVisible)
        .slice(0, maxItems)
        .map((element) => normalizeText(element.textContent))
        .filter(Boolean);
    },
    { limit, selector },
  );
}

export async function visibleClassNames(page, selector, limit = 10) {
  return page.evaluate(
    ({ limit: maxItems, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      return Array.from(document.querySelectorAll(targetSelector))
        .filter(isVisible)
        .slice(0, maxItems)
        .map((element) => (element.className ?? '').toString().trim().replace(/\s+/g, ' '))
        .filter(Boolean);
    },
    { limit, selector },
  );
}

export async function visibleLinkStates(page, selector, limit = 10) {
  return page.evaluate(
    ({ limit: maxItems, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      return Array.from(document.querySelectorAll(targetSelector))
        .filter(isVisible)
        .slice(0, maxItems)
        .map((element) => ({
          href: element.getAttribute('href') ?? '',
          text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
        }));
    },
    { limit, selector },
  );
}

export async function visibleCount(page, selector) {
  return page.evaluate(
    (targetSelector) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      return Array.from(document.querySelectorAll(targetSelector)).filter(isVisible).length;
    },
    selector,
  );
}

export async function visibleTextCount(page, selector, texts) {
  return page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const normalizedTargets = targetTexts.map(normalizeText);
      return Array.from(document.querySelectorAll(targetSelector)).filter((element) => {
        const text = normalizeText(element.textContent);
        return isVisible(element) && normalizedTargets.includes(text);
      }).length;
    },
    { selector, texts: texts.map(normalizeParityText) },
  );
}

export async function waitForVisibleText(page, selector, text) {
  await page.waitForFunction(
    ({ selector: targetSelector, text: targetText }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector)).some((element) => {
        const normalized = normalizeText(element.textContent);
        return isVisible(element) && normalized === targetText;
      });
    },
    { selector, text: normalizeParityText(text) },
    { timeout: 5_000 },
  );
}

export async function waitForPageProperty(page, property, timeout = 5_000) {
  const deadline = Date.now() + timeout;
  while (!page[property]) {
    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for page property ${property}`);
    }
    await page.waitForTimeout(100);
  }
}

export async function legacySelectDropdownState(page, _rootSelector) {
  return page.evaluate(
    ({ dropdownSelector, optionSelector }) => {
      const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      const visible = (selectorText) =>
        Array.from(document.querySelectorAll(selectorText)).filter(isVisible);
      // The antd popup carried presentation-only detail (class, geometry, active/
      // selected item markers) that the shadcn Radix popup expresses differently.
      // Compare only the Tier-1 essence: whether the popup is open and which
      // options it lists.
      return {
        dropdownCount: visible(dropdownSelector).length,
        dropdownItems: visible(optionSelector).map((element) => normalize(element.textContent)),
        viewportWidth: window.innerWidth,
      };
    },
    { dropdownSelector: adminSelectDropdownSelector, optionSelector: adminSelectOptionSelector },
  );
}

export async function activeTabState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const active =
      Array.from(document.querySelectorAll('.ant-tabs-tab-active')).find(isVisible) ??
      Array.from(document.querySelectorAll('.ant-tabs-tab')).find((element) =>
        element.className.includes('active'),
      ) ??
      // Redesigned config page: nav buttons carry aria-current='page' on the
      // active tab instead of an antd active class.
      Array.from(
        document.querySelectorAll('[data-testid^="config-tab-"][aria-current="page"]'),
      ).find(isVisible);
    if (!active) return null;
    return {
      className: normalizeClassName(active.className),
      text: (active.textContent ?? '').trim().replace(/\s+/g, ' '),
    };
  });
}

export async function keyboardFocusState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const element = document.activeElement;
    const label =
      element instanceof HTMLElement
        ? element.closest('.form-group')?.querySelector('label')?.textContent
        : '';

    return {
      ariaLabel: element?.getAttribute?.('aria-label') ?? '',
      className: normalizeClassName(element?.className ?? ''),
      id: element?.id ?? '',
      label: normalize(label),
      name: element?.getAttribute?.('name') ?? '',
      placeholder: element?.getAttribute?.('placeholder') ?? '',
      tag: element?.tagName?.toLowerCase() ?? '',
      text: normalize(element?.textContent).slice(0, 80),
      type: element?.getAttribute?.('type') ?? '',
    };
  });
}

export async function firstElementState(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        !element.closest('.ant-dropdown-hidden')
      );
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const elementState = (element) => ({
      ariaChecked: element.getAttribute('aria-checked'),
      checked: Boolean(element.matches('.ant-switch-checked, [aria-checked="true"], :checked')),
      className: normalizeClassName(element.className),
      disabled: Boolean(element.matches(':disabled, .ant-switch-disabled, .ant-btn-disabled')),
      text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
      value: 'value' in element ? element.value : undefined,
    });
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    if (!element) return null;
    return elementState(element);
  }, selector);
}

export async function firstInputValue(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        !element.closest('.ant-dropdown-hidden')
      );
    };
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    return element && 'value' in element ? element.value : '';
  }, selector);
}

export async function visibleInputValues(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        !element.closest('.ant-dropdown-hidden')
      );
    };
    return Array.from(document.querySelectorAll(targetSelector))
      .filter(isVisible)
      .map((element) => ('value' in element ? element.value : ''));
  }, selector);
}

export async function clickFirstVisible(page, selector) {
  await clickVisibleAt(page, selector, 0);
}

export async function clickFirstVisibleWithPointer(page, selector) {
  const point = await page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        !element.closest('.ant-dropdown-hidden')
      );
    };
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    if (!element) {
      throw new Error(`No visible element ${targetSelector}`);
    }
    element.scrollIntoView({ block: 'center', inline: 'center' });
    const rect = element.getBoundingClientRect();
    return {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    };
  }, selector);
  await page.mouse.click(point.x, point.y);
}

export async function focusFirstVisible(page, selector) {
  await page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    if (!(element instanceof HTMLElement)) {
      throw new Error(`No visible focus target ${targetSelector}`);
    }
    if (!element.hasAttribute('tabindex')) {
      element.setAttribute('tabindex', '-1');
    }
    element.focus();
  }, selector);
}

export async function clickFirstVisibleText(page, selector, texts) {
  const point = await page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const element = Array.from(document.querySelectorAll(targetSelector)).find((candidate) => {
        const text = normalizeText(candidate.textContent);
        return isVisible(candidate) && targetTexts.includes(text);
      });
      if (!element) {
        throw new Error(`No visible element ${targetSelector} with text ${targetTexts.join(', ')}`);
      }
      element.scrollIntoView({ block: 'center', inline: 'center' });
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { selector, texts: texts.map(normalizeParityText) },
  );
  await page.mouse.click(point.x, point.y);
}

// Unlike the coordinate-based helper above, this variant lets Playwright wait
// for the chosen element to stop moving before it clicks. Use it for animated
// dropdown menus: an item can be visible while the overlay is still settling,
// and a cached screen coordinate may otherwise land on an adjacent action.
export async function clickFirstVisibleTextStable(page, selector, texts) {
  const targetTexts = texts.map(normalizeParityText);
  const candidates = page.locator(selector);
  const count = await candidates.count();
  for (let index = 0; index < count; index += 1) {
    const candidate = candidates.nth(index);
    if (!(await candidate.isVisible())) continue;
    const candidateText = normalizeParityText(await candidate.textContent());
    if (!targetTexts.includes(candidateText)) continue;
    await candidate.click();
    return;
  }
  throw new Error(`No visible element ${selector} with text ${targetTexts.join(', ')}`);
}

export async function clickFirstVisibleTextContaining(page, selector, texts) {
  const point = await page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const candidates = Array.from(document.querySelectorAll(targetSelector))
        .filter((candidate) => {
          const text = normalizeText(candidate.textContent);
          return isVisible(candidate) && targetTexts.some((targetText) => text.includes(targetText));
        })
        .sort(
          (left, right) =>
            normalizeText(left.textContent).length - normalizeText(right.textContent).length,
        );
      const element = candidates[0];
      if (!element) {
        throw new Error(
          `No visible element ${targetSelector} containing ${targetTexts.join(', ')}`,
        );
      }
      element.scrollIntoView({ block: 'center', inline: 'center' });
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { selector, texts: texts.map(normalizeParityText) },
  );
  await page.mouse.click(point.x, point.y);
}

export async function clickFirstVisibleTextInViewport(page, selector, texts) {
  const point = await page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const isInViewport = (element) => {
        if (!isVisible(element)) return false;
        const rect = element.getBoundingClientRect();
        return rect.bottom > 0 && rect.right > 0 && rect.top < window.innerHeight && rect.left < window.innerWidth;
      };
      const elements = Array.from(document.querySelectorAll(targetSelector)).filter((candidate) => {
        const text = normalizeText(candidate.textContent);
        return targetTexts.includes(text);
      });
      const element = elements.find(isInViewport) ?? elements.find(isVisible);
      if (!element) {
        throw new Error(`No visible element ${targetSelector} with text ${targetTexts.join(', ')}`);
      }
      element.scrollIntoView({ block: 'center', inline: 'center' });
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { selector, texts: texts.map(normalizeParityText) },
  );
  await page.mouse.click(point.x, point.y);
}

export async function dispatchFirstVisibleTextClick(page, selector, texts) {
  await page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ')
          .replace(/([\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af]) (?=[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff\uac00-\ud7af])/g, '$1');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const element = Array.from(document.querySelectorAll(targetSelector)).find((candidate) => {
        const text = normalizeText(candidate.textContent);
        return isVisible(candidate) && targetTexts.includes(text);
      });
      if (!(element instanceof HTMLElement)) {
        throw new Error(`No visible element ${targetSelector} with text ${targetTexts.join(', ')}`);
      }
      element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    },
    { selector, texts: texts.map(normalizeParityText) },
  );
}

export async function openLegacySelectByLabel(page, rootSelector, labelText) {
  await page.evaluate(
    ({
      labelText: targetLabel,
      rootSelector: targetRoot,
      overlaySelector,
      fieldSelector,
      labelSelector,
      triggerSelector,
    }) => {
      const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      let roots = Array.from(document.querySelectorAll(`${targetRoot}, ${overlaySelector}`));
      if (roots.length === 0) {
        roots = [document.body];
      }
      const clickTrigger = (element) => {
        if (element instanceof HTMLElement) {
          element.click();
          return true;
        }
        return false;
      };
      const visibleTriggerIn = (container) =>
        container ? Array.from(container.querySelectorAll(triggerSelector)).find(isVisible) : null;
      for (const root of roots) {
        // 1. Field container whose label text matches → its select trigger.
        const fields = Array.from(root.querySelectorAll(fieldSelector));
        for (const field of fields) {
          const fieldLabels = Array.from(field.querySelectorAll(labelSelector)).filter(isVisible);
          if (
            !fieldLabels.some((candidate) =>
              normalize(candidate.textContent).includes(targetLabel),
            )
          ) {
            continue;
          }
          if (clickTrigger(visibleTriggerIn(field))) return;
        }

        // 2. shadcn htmlFor → control id (SelectTrigger carries the id).
        const forLabels = Array.from(root.querySelectorAll(labelSelector)).filter(
          (candidate) =>
            isVisible(candidate) && normalize(candidate.textContent).includes(targetLabel),
        );
        for (const label of forLabels) {
          const forId = label.getAttribute('for');
          if (!forId) continue;
          const target = document.getElementById(forId);
          if (!target) continue;
          if (target.matches(triggerSelector) && clickTrigger(target)) return;
          if (clickTrigger(visibleTriggerIn(target))) return;
        }

        const labelCandidates = Array.from(root.querySelectorAll('*'))
          .filter(
            (candidate) =>
              isVisible(candidate) && normalize(candidate.textContent).includes(targetLabel),
          )
          .sort(
            (left, right) =>
              normalize(left.textContent).length - normalize(right.textContent).length,
          );
        for (const candidate of labelCandidates) {
          const containers = [
            candidate.closest(fieldSelector),
            candidate.parentElement,
            candidate.parentElement?.parentElement,
            candidate.closest('.row'),
          ].filter(Boolean);
          for (const container of containers) {
            if (clickTrigger(visibleTriggerIn(container))) return;
          }
        }

        const label = labelCandidates[0];
        if (label) {
          const labelRect = label.getBoundingClientRect();
          const trigger = Array.from(root.querySelectorAll(triggerSelector))
            .filter(isVisible)
            .map((candidate) => {
              const rect = candidate.getBoundingClientRect();
              return {
                element: candidate,
                score:
                  Math.abs(rect.top - labelRect.top) * 4 +
                  Math.max(0, labelRect.left - rect.left) +
                  Math.abs(rect.left - labelRect.left) / 10,
              };
            })
            .sort((left, right) => left.score - right.score)[0]?.element;
          if (clickTrigger(trigger)) return;
        }
      }
      const diagnostics = roots.map((root) => ({
        fields: Array.from(root.querySelectorAll(fieldSelector))
          .slice(0, 30)
          .map((element) => normalize(element.textContent)),
        labels: Array.from(root.querySelectorAll(labelSelector))
          .slice(0, 30)
          .map((element) => normalize(element.textContent)),
        triggers: Array.from(root.querySelectorAll(triggerSelector))
          .filter(isVisible)
          .slice(0, 20)
          .map((element) => normalize(element.textContent)),
      }));
      throw new Error(
        `No visible select with label ${targetLabel} in ${targetRoot}: ${JSON.stringify(
          diagnostics,
        ).slice(0, 3000)}`,
      );
    },
    {
      labelText,
      rootSelector,
      overlaySelector: adminOverlayOpenSelector,
      fieldSelector: adminFormFieldSelector,
      labelSelector: adminFormLabelSelector,
      triggerSelector: adminSelectTriggerSelector,
    },
  );
}

export async function selectLegacyFormOption(
  page,
  rootSelector,
  labelText,
  optionTexts,
  { waitForHidden = true } = {},
) {
  try {
    await openLegacySelectByLabel(page, rootSelector, labelText);
    await waitForVisibleText(page, adminSelectOptionSelector, optionTexts[0]);
    await clickFirstVisibleTextStable(page, adminSelectOptionSelector, optionTexts);
    if (waitForHidden) {
      try {
        await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
      } catch {
        await page.mouse.click(1, 1).catch(() => undefined);
        await page.waitForTimeout(150);
        await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
      }
    } else {
      await page.waitForTimeout(100);
    }
  } catch (error) {
    throw new Error(
      `Failed selecting ${labelText} -> ${optionTexts.join(' / ')}: ${error.message}`,
    );
  }
}

export async function waitForVisibleElementsHidden(page, selector, timeout = 5_000) {
  await page.waitForFunction(
    (targetSelector) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return !Array.from(document.querySelectorAll(targetSelector)).some(isVisible);
    },
    selector,
    { timeout },
  );
}

export async function waitForVisibleElementCountAtLeast(page, selector, minCount, timeout = 5_000) {
  await page.waitForFunction(
    ({ minCount: targetMinCount, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return (
        Array.from(document.querySelectorAll(targetSelector)).filter(isVisible).length >=
        targetMinCount
      );
    },
    { minCount, selector },
    { timeout },
  );
}

export async function waitForPagePropertyAtLeast(page, property, minimum, timeout = 5_000) {
  const startedAt = Date.now();
  while ((page[property] ?? 0) < minimum) {
    if (Date.now() - startedAt > timeout) {
      throw new Error(`${property} did not reach ${minimum}`);
    }
    await delay(50);
  }
}

export async function clickVisibleAt(page, selector, index) {
  await page.evaluate(
    ({ index: targetIndex, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const element = Array.from(document.querySelectorAll(targetSelector)).filter(isVisible)[
        targetIndex
      ];
      if (!element) {
        throw new Error(`No visible element ${targetSelector} at index ${targetIndex}`);
      }
      element.click();
    },
    { index, selector },
  );
}

export async function visibleElementDomIndex(page, selector, index) {
  return page.evaluate(
    ({ index: targetIndex, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const elements = Array.from(document.querySelectorAll(targetSelector));
      const element = elements.filter(isVisible)[targetIndex];
      if (!element) {
        throw new Error(`No visible element ${targetSelector} at index ${targetIndex}`);
      }
      return elements.indexOf(element);
    },
    { index, selector },
  );
}

export async function safeVisibleElementDomIndex(page, selector, index) {
  try {
    return await visibleElementDomIndex(page, selector, index);
  } catch {
    return -1;
  }
}

export async function fillFirstVisible(page, selector, value) {
  await fillVisibleAt(page, selector, 0, value);
}

export async function fillFirstVisibleIfPresent(page, selector, value) {
  try {
    await fillFirstVisible(page, selector, value);
  } catch {
    // The packaged knowledge oracle has no search box; redesigned source keeps one.
  }
}

export async function waitForVisibleInputByLabel(page, rootSelector, labelText, timeout = 5_000) {
  await page.waitForFunction(
    ({ labelText: targetLabelText, rootSelector: targetRootSelector, fieldSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      const root = Array.from(document.querySelectorAll(targetRootSelector)).find(isVisible);
      const group = root
        ? Array.from(root.querySelectorAll(fieldSelector)).find(
            (element) =>
              isVisible(element) &&
              Array.from(element.querySelectorAll('label, [data-slot="label"]')).some((label) =>
                (label.textContent ?? '').includes(targetLabelText),
              ),
          )
        : null;
      return Boolean(
        group &&
          Array.from(group.querySelectorAll('input, textarea')).some(
            (element) => isVisible(element) && !element.className.includes('ant-select-search__field'),
          ),
      );
    },
    { labelText, rootSelector, fieldSelector: adminFormFieldSelector },
    { timeout },
  );
}

export async function fillVisibleInputByLabel(page, rootSelector, labelText, value) {
  const domIndex = await page.evaluate(
    ({ labelText: targetLabelText, rootSelector: targetRootSelector, fieldSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      const root = Array.from(document.querySelectorAll(targetRootSelector)).find(isVisible);
      const group = root
        ? Array.from(root.querySelectorAll(fieldSelector)).find(
            (element) =>
              isVisible(element) &&
              Array.from(element.querySelectorAll('label, [data-slot="label"]')).some((label) =>
                (label.textContent ?? '').includes(targetLabelText),
              ),
          )
        : null;
      const input = group
        ? Array.from(group.querySelectorAll('input, textarea')).find(
            (element) => isVisible(element) && !element.className.includes('ant-select-search__field'),
          )
        : null;
      if (!(input instanceof HTMLInputElement || input instanceof HTMLTextAreaElement)) {
        throw new Error(`No visible input for label ${targetLabelText}`);
      }
      return Array.from(document.querySelectorAll('input, textarea')).indexOf(input);
    },
    { labelText, rootSelector, fieldSelector: adminFormFieldSelector },
  );
  await page.locator('input, textarea').nth(domIndex).fill(value);
}

export async function fillVisibleAt(page, selector, index, value) {
  const domIndex = await visibleElementDomIndex(page, selector, index);
  await page.locator(selector).nth(domIndex).fill(value);
}

export async function blurVisibleAt(page, selector, index) {
  const domIndex = await visibleElementDomIndex(page, selector, index);
  await page.locator(selector).nth(domIndex).blur();
}

export async function readDebugSnapshot(page) {
  const [title, body] = await Promise.all([
    page.title().catch((error) => `title error: ${error.message}`),
    page
      .locator('body')
      .innerText({ timeout: 1_000 })
      .catch((error) => `body error: ${error.message}`),
  ]);
  return {
    body: body.trim().replace(/\s+/g, ' ').slice(0, 500),
    title,
    url: page.url(),
  };
}

export async function waitForReadySelector(page, selector, diagnostics = [], timeout = 10_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const visible = await page.evaluate((readySelector) => {
        const element = document.querySelector(readySelector);
        if (!element) return false;
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      }, selector);
      if (visible) return;
    } catch (error) {
      lastError = error;
      diagnostics.push(`ready selector retry ${selector}: ${error.message}`);
    }
    await page.waitForTimeout(100);
  }
  throw lastError ?? new Error(`Ready selector ${selector} did not become visible`);
}
