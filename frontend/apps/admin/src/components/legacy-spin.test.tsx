import { act } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, vi } from 'vitest';
import { LegacySpin } from './legacy-spin';

describe('LegacySpin', () => {
  it('renders the old nested loading structure without Ant Design v5 Spin', () => {
    const idle = renderToStaticMarkup(
      <LegacySpin loading={false}>
        <button type="button">编辑排序</button>
      </LegacySpin>,
    );

    expect(idle).toContain('class="ant-spin-nested-loading"');
    expect(idle).toContain('class="ant-spin-container"');
    expect(idle).not.toContain('class="ant-spin"');
    expect(idle).not.toContain('class="spinner-grow text-primary"');
    expect(idle).toContain('<button type="button">编辑排序</button>');
    expect(idle).not.toContain('css-dev-only-do-not-override');

    const loading = renderToStaticMarkup(
      <LegacySpin loading>
        <span>content</span>
      </LegacySpin>,
    );

    expect(loading).toContain('class="ant-spin ant-spin-spinning"');
    expect(loading).toContain('class="spinner-grow text-primary ant-spin-dot"');
    expect(loading).toContain('class="ant-spin-container ant-spin-blur"');
  });

  it('keeps the old Ant v3 nested class, size, tip, style, and prop placement', () => {
    const html = renderToStaticMarkup(
      <LegacySpin
        loading
        className="inner-spin"
        data-state="loading"
        size="large"
        style={{ marginLeft: 4 }}
        tip="加载中"
        wrapperClassName="outer-spin"
      >
        <span>content</span>
      </LegacySpin>,
    );

    expect(html).toContain('class="ant-spin-nested-loading outer-spin"');
    expect(html).toContain('data-state="loading"');
    expect(html).toContain(
      'class="ant-spin ant-spin-lg ant-spin-spinning ant-spin-show-text inner-spin"',
    );
    expect(html).toContain('style="margin-left:4px"');
    expect(html).toContain('class="ant-spin-text">加载中</div>');
    expect(html).toContain('class="ant-spin-container ant-spin-blur"');
  });

  it('renders the old standalone Spin shape when there are no nested children', () => {
    const html = renderToStaticMarkup(<LegacySpin loading={false} />);

    expect(html).toContain('class="ant-spin"');
    expect(html).toContain('class="spinner-grow text-primary ant-spin-dot"');
    expect(html).not.toContain('ant-spin-nested-loading');
    expect(html).not.toContain('ant-spin-spinning');
  });

  it('honors the old delayed spinning behavior', async () => {
    vi.useFakeTimers();
    const container = document.createElement('div');
    const root = createRoot(container);
    document.body.appendChild(container);

    try {
      await act(async () => {
        root.render(
          <LegacySpin loading delay={100}>
            <span>content</span>
          </LegacySpin>,
        );
      });
      expect(container.querySelector('.ant-spin')).toBeNull();
      expect(container.querySelector('.ant-spin-container')?.className).toBe('ant-spin-container');

      await act(async () => {
        vi.advanceTimersByTime(99);
      });
      expect(container.querySelector('.ant-spin')).toBeNull();

      await act(async () => {
        vi.advanceTimersByTime(1);
      });
      expect(container.querySelector('.ant-spin')?.className).toBe('ant-spin ant-spin-spinning');
      expect(container.querySelector('.ant-spin-container')?.className).toBe(
        'ant-spin-container ant-spin-blur',
      );

      await act(async () => {
        root.render(
          <LegacySpin loading={false} delay={100}>
            <span>content</span>
          </LegacySpin>,
        );
      });
      expect(container.querySelector('.ant-spin')).toBeNull();
      expect(container.querySelector('.ant-spin-container')?.className).toBe('ant-spin-container');
    } finally {
      await act(async () => root.unmount());
      container.remove();
      vi.useRealTimers();
    }
  });
});
