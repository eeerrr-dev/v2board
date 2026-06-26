import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { FormField } from './form-field';
import { Input } from './input';

describe('FormField', () => {
  it('labels the control and wires the shared id', () => {
    const html = renderToStaticMarkup(
      <FormField id="login-email" label="Email">
        <Input type="email" />
      </FormField>,
    );
    expect(html).toContain('for="login-email"');
    expect(html).toContain('id="login-email"');
    expect(html).toContain('Email');
    expect(html).toContain('type="email"');
  });

  it('renders an error with role=alert and marks the control invalid + described-by', () => {
    const html = renderToStaticMarkup(
      <FormField id="pw" label="Password" error="Required">
        <Input type="password" />
      </FormField>,
    );
    expect(html).toContain('role="alert"');
    expect(html).toContain('id="pw-error"');
    expect(html).toContain('aria-describedby="pw-error"');
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain('border-destructive');
    expect(html).toContain('Required');
  });

  it('associates a description through aria-describedby', () => {
    const html = renderToStaticMarkup(
      <FormField id="email" label="Email" description="We never share it">
        <Input />
      </FormField>,
    );
    expect(html).toContain('id="email-description"');
    expect(html).toContain('aria-describedby="email-description"');
    expect(html).toContain('We never share it');
  });

  it('owns the control id so the label association always matches', () => {
    const html = renderToStaticMarkup(
      <FormField id="field" label="Email">
        <Input id="ignored" />
      </FormField>,
    );
    expect(html).toContain('for="field"');
    expect(html).toContain('id="field"');
    expect(html).not.toContain('ignored');
  });
});
