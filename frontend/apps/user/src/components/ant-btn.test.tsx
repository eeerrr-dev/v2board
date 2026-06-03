import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AntBtn } from './ant-btn';

describe('AntBtn legacy child rendering', () => {
  it('wraps string labels like antd Button', () => {
    const html = renderToStaticMarkup(<AntBtn className="ant-btn">充值</AntBtn>);

    expect(html).toContain('<span>充 值</span>');
  });

  it('leaves numeric children unwrapped like antd Button', () => {
    const html = renderToStaticMarkup(<AntBtn className="ant-btn">{1}</AntBtn>);

    expect(html).toBe('<button type="button" class="ant-btn">1</button>');
  });
});
