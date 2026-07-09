import { act } from 'react';
import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useForm, type UseFormReturn } from 'react-hook-form';
import { renderWithProviders } from '@/test/render';
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from './form';
import { Input } from './input';

// FormMessage resolves the resolver-stashed i18n key through useTranslation.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'errors.required' ? 'This field is required' : key),
  }),
}));

type Values = { email: string };

let form: UseFormReturn<Values> | undefined;

function TestForm() {
  form = useForm<Values>({ defaultValues: { email: '' } });
  return (
    <Form {...form}>
      <FormField
        control={form.control}
        name="email"
        render={({ field }) => (
          <FormItem>
            <FormLabel>Email</FormLabel>
            <FormControl>
              <Input type="email" {...field} />
            </FormControl>
            <FormDescription>We never share it</FormDescription>
            <FormMessage />
          </FormItem>
        )}
      />
    </Form>
  );
}

describe('Form (shadcn RHF primitives)', () => {
  beforeEach(() => {
    renderWithProviders(<TestForm />);
  });

  afterEach(() => {
    form = undefined;
  });

  it('wires label/control/description ids in the resting state', () => {
    // The accessible name/description prove the label `for` and
    // `aria-describedby` id wiring end to end.
    const input = screen.getByRole('textbox', { name: 'Email' });

    expect(input).toHaveAttribute('aria-invalid', 'false');
    expect(input).toHaveAccessibleDescription('We never share it');
    expect(screen.queryByText('This field is required')).not.toBeInTheDocument();
  });

  it('marks the control invalid, extends the description, and renders the translated message on error', async () => {
    await act(async () => {
      form!.setError('email', { type: 'manual', message: 'errors.required' });
    });

    const input = screen.getByRole('textbox', { name: 'Email' });
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription('We never share it This field is required');
    expect(screen.getByText('This field is required')).toBeInTheDocument();
    expect(screen.getByText('Email')).toHaveAttribute('data-error', 'true');
  });

  it('clears the message and invalid state when the error is removed', async () => {
    await act(async () => {
      form!.setError('email', { type: 'manual', message: 'errors.required' });
    });
    await act(async () => {
      form!.clearErrors('email');
    });

    const input = screen.getByRole('textbox', { name: 'Email' });
    expect(input).toHaveAttribute('aria-invalid', 'false');
    expect(screen.queryByText('This field is required')).not.toBeInTheDocument();
  });
});
