import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useForm, type UseFormReturn } from 'react-hook-form';
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

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    await act(async () => {
      root.render(<TestForm />);
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    form = undefined;
  });

  it('wires label/control/description ids in the resting state', () => {
    const input = container.querySelector('input')!;
    const label = container.querySelector('label')!;
    const description = container.querySelector('[data-slot="form-description"]')!;

    expect(input.id).not.toBe('');
    expect(label.getAttribute('for')).toBe(input.id);
    expect(input.getAttribute('aria-invalid')).toBe('false');
    expect(input.getAttribute('aria-describedby')).toBe(description.id);
    expect(container.querySelector('[data-slot="form-message"]')).toBeNull();
  });

  it('marks the control invalid, extends aria-describedby, and renders the translated message on error', async () => {
    await act(async () => {
      form!.setError('email', { type: 'manual', message: 'errors.required' });
    });

    const input = container.querySelector('input')!;
    const description = container.querySelector('[data-slot="form-description"]')!;
    const message = container.querySelector('[data-slot="form-message"]')!;

    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(message.id).not.toBe('');
    expect(input.getAttribute('aria-describedby')).toBe(`${description.id} ${message.id}`);
    expect(message.textContent).toBe('This field is required');
    expect(container.querySelector('label')!.getAttribute('data-error')).toBe('true');
  });

  it('clears the message and invalid state when the error is removed', async () => {
    await act(async () => {
      form!.setError('email', { type: 'manual', message: 'errors.required' });
    });
    await act(async () => {
      form!.clearErrors('email');
    });

    const input = container.querySelector('input')!;
    expect(input.getAttribute('aria-invalid')).toBe('false');
    expect(container.querySelector('[data-slot="form-message"]')).toBeNull();
  });
});
