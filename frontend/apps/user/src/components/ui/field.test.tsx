import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Field, FieldDescription, FieldError, FieldLabel } from './field';
import { createTestTranslation } from '@/test/i18next-selector';

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation({ 'errors.required': 'This field is required' }),
}));

describe('FieldError', () => {
  it('translates a resolver-provided i18n key', () => {
    render(<FieldError errors={[{ message: 'errors.required' }]} />);

    expect(screen.getByRole('alert')).toHaveTextContent('This field is required');
  });

  it('preserves a literal Chinese resolver message', () => {
    render(<FieldError errors={[{ message: '请输入礼品卡' }]} />);

    expect(screen.getByRole('alert')).toHaveTextContent('请输入礼品卡');
  });

  it('associates the field group with its label, description, and active error', async () => {
    render(
      <>
        <p id="external-help">External help</p>
        <Field data-invalid aria-describedby="external-help">
          <FieldLabel id="email-label" htmlFor="email">
            Email
          </FieldLabel>
          <input id="email" aria-invalid="true" />
          <FieldDescription id="email-description">Use a work address</FieldDescription>
          <FieldError id="email-error">Invalid address</FieldError>
        </Field>
      </>,
    );

    const field = screen.getByRole('group');
    await waitFor(() => {
      expect(field).toHaveAttribute(
        'aria-describedby',
        'external-help email-description email-error',
      );
    });
    expect(field).toHaveAttribute('aria-errormessage', 'email-error');
    expect(field).toHaveAttribute('aria-invalid', 'true');
  });
});
