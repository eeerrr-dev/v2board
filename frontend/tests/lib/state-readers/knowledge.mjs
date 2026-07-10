import { visibleTexts, visibleCount, firstInputValue } from '../dom-helpers.mjs';

export async function knowledgeState(page) {
  return {
    articleTitles: await visibleTexts(
      page,
      '[data-testid="knowledge-item-title"], .list-group-item h5',
      8,
    ),
    categoryTitles: await visibleTexts(
      page,
      '[data-testid="knowledge-category-title"], .block-header .block-title',
      8,
    ),
    drawerBodies: await visibleTexts(
      page,
      '[data-testid="knowledge-sheet-body"] .custom-html-style, .ant-drawer-body .custom-html-style',
      4,
    ),
    drawerOpenCount: await visibleCount(page, '[data-testid="knowledge-sheet"], .ant-drawer-open'),
    drawerTitles: await visibleTexts(
      page,
      '[data-testid="knowledge-sheet-title"], .ant-drawer-title',
      4,
    ),
    searchValue: await firstInputValue(page, '[data-testid="knowledge-search-bar"] input'),
  };
}
