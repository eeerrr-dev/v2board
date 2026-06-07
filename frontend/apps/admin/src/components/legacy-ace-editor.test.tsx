import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { LegacyAceJsonEditor } from './legacy-ace-editor';

describe('LegacyAceJsonEditor', () => {
  it('renders the old ReactAce json/github editor shell', () => {
    const html = renderToStaticMarkup(
      <LegacyAceJsonEditor value={'{\n  "tag": "ss_out"\n}'} placeholder="{}" />,
    );

    expect(html).toContain('class="ace_editor ace-github"');
    expect(html).toContain('style="width:500px;height:500px;font-size:14px"');
    expect(html).toContain('data-legacy-mode="json"');
    expect(html).toContain('data-legacy-theme="github"');
    expect(html).toContain('data-legacy-show-print-margin="true"');
    expect(html).toContain('data-legacy-show-gutter="true"');
    expect(html).toContain('data-legacy-highlight-active-line="true"');
    expect(html).toContain('data-legacy-enable-basic-autocompletion="false"');
    expect(html).toContain('data-legacy-enable-live-autocompletion="false"');
    expect(html).toContain('data-legacy-enable-snippets="false"');
    expect(html).toContain('data-legacy-show-line-numbers="true"');
    expect(html).toContain('data-legacy-tab-size="2"');
    expect(html).toContain('class="ace_gutter"');
    expect(html).toContain('class="ace_scroller"');
    expect(html).toContain('class="ace_layer ace_print-margin-layer"');
    expect(html).toContain('class="ace_text-input legacy-ace-json-input"');
    expect(html).not.toContain('class="ant-input"');
    expect(html).not.toContain('rows="8"');
  });
});
