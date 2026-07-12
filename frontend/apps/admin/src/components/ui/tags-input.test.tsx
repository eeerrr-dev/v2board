import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { useState } from 'react';
import { TagsInput } from './tags-input';

function ControlledTagsInput({ initial = [] }: { initial?: string[] }) {
  const [value, setValue] = useState(initial);
  return <TagsInput aria-label="标签" value={value} onChange={setValue} />;
}

describe('TagsInput', () => {
  it('commits trimmed unique tags with Enter and removes a tag button', async () => {
    const user = userEvent.setup();
    render(<ControlledTagsInput initial={['existing']} />);
    const input = screen.getByLabelText('标签');

    await user.type(input, '  new tag  {Enter}');
    expect(screen.getByText('new tag')).toBeInTheDocument();

    await user.type(input, 'existing{Enter}');
    expect(screen.getAllByText('existing')).toHaveLength(1);

    await user.click(screen.getByLabelText('移除标签 existing'));
    expect(screen.queryByText('existing')).not.toBeInTheDocument();
  });

  it('uses Backspace on an empty draft to remove only the last tag', async () => {
    const user = userEvent.setup();
    render(<ControlledTagsInput initial={['one', 'two']} />);
    const input = screen.getByLabelText('标签');

    fireEvent.keyDown(input, { key: 'Backspace', isComposing: true });
    expect(screen.getByText('two')).toBeInTheDocument();

    await user.type(input, '{Backspace}');
    expect(screen.queryByText('two')).not.toBeInTheDocument();
    expect(screen.getByText('one')).toBeInTheDocument();

    await user.type(input, 'draft{Backspace}');
    expect(screen.getByText('one')).toBeInTheDocument();
    expect(input).toHaveValue('draf');
  });

  it('commits on blur and exposes invalid ARIA state', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const onBlur = vi.fn();
    render(<TagsInput aria-label="标签" value={[]} onChange={onChange} onBlur={onBlur} invalid />);

    const input = screen.getByLabelText('标签');
    await user.type(input, 'blur tag');
    await user.tab();

    expect(onChange).toHaveBeenCalledWith(['blur tag']);
    expect(onBlur).toHaveBeenCalledOnce();
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input.closest('[data-slot="tags-input"]')).toHaveAttribute('data-invalid', 'true');
  });
});
