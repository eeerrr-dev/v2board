function escapeHtml(value: string) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function renderInline(value: string) {
  return value
    .replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img alt="$1" src="$2">')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/\*([^*]+)\*/g, '<em>$1</em>')
    .replace(/(^|[\s>])((?:https?:\/\/|mailto:)[^\s<]+)/g, '$1<a href="$2" target="_blank" rel="noreferrer">$2</a>');
}

function renderTable(lines: string[]) {
  const rows = lines.map((line) =>
    line
      .trim()
      .replace(/^\|/, '')
      .replace(/\|$/, '')
      .split('|')
      .map((cell) => renderInline(cell.trim())),
  );
  const [head, , ...body] = rows;
  if (!head) return '';
  return [
    '<table><thead><tr>',
    head.map((cell) => `<th>${cell}</th>`).join(''),
    '</tr></thead><tbody>',
    body.map((row) => `<tr>${row.map((cell) => `<td>${cell}</td>`).join('')}</tr>`).join(''),
    '</tbody></table>',
  ].join('');
}

export function renderLegacyMarkdown(markdown: string) {
  const normalized = markdown.replace(/\r\n?/g, '\n');
  const lines = normalized.split('\n');
  const html: string[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index] ?? '';
    if (!line.trim()) {
      index += 1;
      continue;
    }

    if (line.startsWith('```')) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index]!.startsWith('```')) {
        code.push(lines[index]!);
        index += 1;
      }
      index += 1;
      html.push(`<pre><code>${escapeHtml(code.join('\n'))}</code></pre>`);
      continue;
    }

    if (/^<[^>]+>/.test(line.trim())) {
      const raw: string[] = [line];
      index += 1;
      while (index < lines.length && lines[index]?.trim()) {
        raw.push(lines[index]!);
        index += 1;
      }
      html.push(raw.join('\n'));
      continue;
    }

    if (/^\|.+\|$/.test(line.trim()) && /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(lines[index + 1] ?? '')) {
      const table: string[] = [line, lines[index + 1]!];
      index += 2;
      while (index < lines.length && /^\|.+\|$/.test(lines[index]!.trim())) {
        table.push(lines[index]!);
        index += 1;
      }
      html.push(renderTable(table));
      continue;
    }

    const heading = /^(#{1,6})\s+(.+)$/.exec(line);
    if (heading) {
      const level = heading[1]!.length;
      html.push(`<h${level}>${renderInline(heading[2]!)}</h${level}>`);
      index += 1;
      continue;
    }

    if (/^>\s?/.test(line)) {
      const quote: string[] = [];
      while (index < lines.length && /^>\s?/.test(lines[index]!)) {
        quote.push(lines[index]!.replace(/^>\s?/, ''));
        index += 1;
      }
      html.push(`<blockquote>${renderLegacyMarkdown(quote.join('\n'))}</blockquote>`);
      continue;
    }

    const unordered = /^\s*[-*+]\s+/.test(line);
    const ordered = /^\s*\d+\.\s+/.test(line);
    if (unordered || ordered) {
      const tag = ordered ? 'ol' : 'ul';
      const items: string[] = [];
      const pattern = ordered ? /^\s*\d+\.\s+/ : /^\s*[-*+]\s+/;
      while (index < lines.length && pattern.test(lines[index]!)) {
        items.push(`<li>${renderInline(lines[index]!.replace(pattern, ''))}</li>`);
        index += 1;
      }
      html.push(`<${tag}>${items.join('')}</${tag}>`);
      continue;
    }

    const paragraph: string[] = [line];
    index += 1;
    while (
      index < lines.length &&
      lines[index]?.trim() &&
      !/^(#{1,6})\s+/.test(lines[index]!) &&
      !/^(```|>\s?|\s*[-*+]\s+|\s*\d+\.\s+)/.test(lines[index]!)
    ) {
      paragraph.push(lines[index]!);
      index += 1;
    }
    html.push(`<p>${renderInline(paragraph.join('\n'))}</p>`);
  }

  return html.join('\n');
}
