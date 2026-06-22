import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it } from 'vitest';
import { syncFixedColumnRowHeights } from './use-fixed-column-row-heights';

const source = readFileSync('src/lib/use-fixed-column-row-heights.ts', 'utf8');

function setRect(element: Element | null, rect: { height: number; width?: number }): void {
  if (!element) throw new Error('Missing test element');
  Object.defineProperty(element, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ height: rect.height, width: rect.width ?? 0 }),
  });
}

function setHeight(element: Element | null, height: number): void {
  setRect(element, { height });
}

describe('syncFixedColumnRowHeights', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('matches admin fixed body rows to main row heights by data-row-key', () => {
    document.body.innerHTML = `
      <div class="ant-table">
        <table id="main">
          <thead><tr id="main-head"><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="a" id="main-a"><td>A</td></tr>
            <tr class="ant-table-row" data-row-key="b" id="main-b"><td>B</td></tr>
          </tbody>
        </table>
        <table id="fixed">
          <thead><tr id="fixed-head"><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="b" id="fixed-b" style="height: 1px"><td>B</td></tr>
            <tr class="ant-table-row" data-row-key="a" id="fixed-a" style="height: 1px"><td>A</td></tr>
          </tbody>
        </table>
      </div>
    `;

    setRect(document.querySelector('.ant-table'), { height: 180, width: 390 });
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main-a'), 96);
    setHeight(document.querySelector('#main-b'), 54);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
    );

    expect(document.querySelector<HTMLElement>('#fixed-head')?.style.height).toBe('54px');
    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('96px');
    expect(document.querySelector<HTMLElement>('#fixed-b')?.style.height).toBe('54px');
  });

  it('does not stamp row heights while the admin table is hidden', () => {
    document.body.innerHTML = `
      <div class="ant-table">
        <table id="main">
          <thead><tr><th>Action</th></tr></thead>
          <tbody><tr class="ant-table-row" data-row-key="a"><td>A</td></tr></tbody>
        </table>
        <table id="fixed">
          <thead><tr id="fixed-head"><th>Action</th></tr></thead>
          <tbody><tr class="ant-table-row" data-row-key="a" id="fixed-a"><td>A</td></tr></tbody>
        </table>
      </div>
    `;

    setHeight(document.querySelector('.ant-table'), 0);
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main .ant-table-row'), 96);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
    );

    expect(document.querySelector<HTMLElement>('#fixed-head')?.style.height).toBe('');
    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('');
  });
});

describe('useFixedColumnRowHeights', () => {
  it('resyncs after frame and font readiness like legacy Ant Table', () => {
    expect(source).toContain('window.requestAnimationFrame(sync)');
    expect(source).toContain('document.fonts?.ready');
    expect(source).toContain('fontsReady?.then');
  });
});
