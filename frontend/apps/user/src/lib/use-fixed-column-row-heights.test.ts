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

  it('matches fixed body rows to main body row heights by data-row-key', () => {
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

    setRect(document.querySelector('.ant-table'), { height: 180, width: 900 });
    setRect(document.querySelector('#main'), { height: 180, width: 900 });
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main-a'), 60);
    setHeight(document.querySelector('#main-b'), 40);
    setHeight(document.querySelector('#fixed-a'), 61);
    setHeight(document.querySelector('#fixed-b'), 55);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
    );

    expect(document.querySelector<HTMLElement>('#fixed-head')?.style.height).toBe('54px');
    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('60px');
    expect(document.querySelector<HTMLElement>('#fixed-b')?.style.height).toBe('40px');
  });

  it('matches main body row heights while the table is horizontally clipped', () => {
    document.body.innerHTML = `
      <div class="ant-table" id="table">
        <table id="main">
          <thead><tr><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="a" id="main-a"><td>A</td></tr>
            <tr class="ant-table-row" data-row-key="b" id="main-b"><td>B</td></tr>
          </tbody>
        </table>
        <table id="fixed">
          <thead><tr id="fixed-head"><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="b" id="fixed-b"><td>B</td></tr>
            <tr class="ant-table-row" data-row-key="a" id="fixed-a"><td>A</td></tr>
          </tbody>
        </table>
      </div>
    `;

    setRect(document.querySelector('#table'), { height: 180, width: 390 });
    setRect(document.querySelector('#main'), { height: 180, width: 900 });
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main-a'), 60);
    setHeight(document.querySelector('#main-b'), 40);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
    );

    expect(document.querySelector<HTMLElement>('#fixed-head')?.style.height).toBe('54px');
    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('60px');
    expect(document.querySelector<HTMLElement>('#fixed-b')?.style.height).toBe('40px');
  });

  it('can apply the legacy fixed-body offset used by bordered table wrappers', () => {
    document.body.innerHTML = `
      <div class="ant-table">
        <table id="main">
          <thead><tr><th>Action</th></tr></thead>
          <tbody><tr class="ant-table-row" data-row-key="a"><td>A</td></tr></tbody>
        </table>
        <table id="fixed">
          <thead><tr><th>Action</th></tr></thead>
          <tbody><tr class="ant-table-row" data-row-key="a" id="fixed-a"><td>A</td></tr></tbody>
        </table>
      </div>
    `;

    setHeight(document.querySelector('.ant-table'), 180);
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main .ant-table-row'), 54);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
      { bodyRowHeightOffset: 1 },
    );

    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('55px');
  });

  it('limits the legacy fixed-body offset to rows at or under the source-height threshold', () => {
    document.body.innerHTML = `
      <div class="ant-table">
        <table id="main">
          <thead><tr><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="plain" id="main-plain"><td>Plain</td></tr>
            <tr class="ant-table-row" data-row-key="wrapped" id="main-wrapped"><td>Wrapped</td></tr>
          </tbody>
        </table>
        <table id="fixed">
          <thead><tr><th>Action</th></tr></thead>
          <tbody>
            <tr class="ant-table-row" data-row-key="plain" id="fixed-plain"><td>Plain</td></tr>
            <tr class="ant-table-row" data-row-key="wrapped" id="fixed-wrapped"><td>Wrapped</td></tr>
          </tbody>
        </table>
      </div>
    `;

    setHeight(document.querySelector('.ant-table'), 180);
    setHeight(document.querySelector('#main thead'), 54);
    setHeight(document.querySelector('#main-plain'), 54);
    setHeight(document.querySelector('#main-wrapped'), 75);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
      { bodyRowHeightOffset: 1, bodyRowHeightOffsetMaxSourceHeight: 54 },
    );

    expect(document.querySelector<HTMLElement>('#fixed-plain')?.style.height).toBe('55px');
    expect(document.querySelector<HTMLElement>('#fixed-wrapped')?.style.height).toBe('75px');
  });

  it('does not stamp row heights while the table is hidden', () => {
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
    setHeight(document.querySelector('#main .ant-table-row'), 60);
    setHeight(document.querySelector('#fixed-a'), 61);

    syncFixedColumnRowHeights(
      document.querySelector<HTMLTableElement>('#main')!,
      document.querySelector<HTMLTableElement>('#fixed')!,
    );

    expect(document.querySelector<HTMLElement>('#fixed-head')?.style.height).toBe('');
    expect(document.querySelector<HTMLElement>('#fixed-a')?.style.height).toBe('');
  });

});

describe('useFixedColumnRowHeights', () => {
  it('resyncs fixed rows after frame and font readiness like the old table lifecycle', () => {
    expect(source).toContain('window.requestAnimationFrame(sync)');
    expect(source).toContain('document.fonts?.ready');
    expect(source).toContain('fontsReady?.then');
  });

  it('does not carry the removed fixedBodyRowExtraPixel compatibility option', () => {
    expect(source).not.toContain('fixedBodyRowExtraPixel');
  });
});
