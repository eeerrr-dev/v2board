import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default tseslint.config(
  {
    ignores: [
      '**/.cache/**',
      '**/coverage/**',
      '**/dist/**',
      '**/dist-deploy/**',
      '**/node_modules/**',
      '**/*.config.{ts,js}',
    ],
  },
  js.configs.recommended,
  {
    files: ['scripts/**/*.mjs'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.node },
    },
    rules: {
      'no-console': 'off',
      'no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      'preserve-caught-error': 'off',
    },
  },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [tseslint.configs.recommended],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaVersion: 'latest', sourceType: 'module', ecmaFeatures: { jsx: true } },
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: {
      react,
      'react-hooks': reactHooks,
    },
    settings: { react: { version: '19.2' } },
    rules: {
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/consistent-type-imports': ['error', { prefer: 'type-imports' }],
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-unused-expressions': [
        'error',
        { allowShortCircuit: true, allowTernary: true },
      ],
      'no-console': 'off',
      'no-empty': ['error', { allowEmptyCatch: true }],
      'no-undef': 'off',
      'no-unused-expressions': ['error', { allowShortCircuit: true, allowTernary: true }],
      'react/jsx-key': 'off',
      'react/no-unknown-property': ['error', { ignore: ['unselectable'] }],
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',
      'react/jsx-no-target-blank': 'off',
      'react-hooks/exhaustive-deps': 'off',
      'react-hooks/immutability': 'off',
      'react-hooks/incompatible-library': 'off',
      'react-hooks/purity': 'off',
      'react-hooks/refs': 'off',
      'react-hooks/set-state-in-effect': 'off',
    },
  },
  {
    files: ['**/*.d.ts'],
    rules: {
      '@typescript-eslint/no-unused-vars': 'off',
    },
  },
  // Type-aware deprecation guard. Flags any NEW usage of an @deprecated API in
  // authored app + package source. It needs type information, so this block (and only
  // this block) turns on `projectService`; it is scoped to `*/src` so it never touches
  // files outside a tsconfig. Pure-type deprecations have been cleared (FormEvent ->
  // SyntheticEvent, ElementRef -> ComponentRef, MutableRefObject -> RefObject). The
  // behavioral deprecations deliberately retained for legacy parity (keyCode,
  // onKeyPress, execCommand, clip, webkitUserSelect, getElementsByTagName, substr,
  // unescape, returnValue, charCode) each carry an inline `eslint-disable-next-line
  // @typescript-eslint/no-deprecated -- behavior-parity` directive at their call site
  // (AGENTS.md behavior-parity gate). Drop those directives as each surface is
  // redesigned and parity-retired.
  {
    files: ['apps/*/src/**/*.{ts,tsx}', 'packages/*/src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        projectService: true,
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
    },
    rules: { '@typescript-eslint/no-deprecated': 'error' },
  },
  // The user app runs the React Compiler (1.0), so the compiler-correctness
  // react-hooks rules are turned back on for its source (they stay off globally
  // because the admin replica is still legacy DOM/hook patterns).
  //
  // Enforced: purity / refs / immutability / incompatible-library — these are the
  // rules the compiler actually relies on to safely auto-memoize. They are clean
  // across the user app, except TanStack Table's useReactTable (an inherently
  // non-memoizable API), disabled inline at that one call site.
  //
  // Deliberately NOT enabled here: exhaustive-deps and set-state-in-effect. Both
  // are advisory hygiene rules the compiler tolerates, and every current violation
  // is an intentional, behavior-parity effect on a contract-covered surface — the
  // backend `弹窗` auto-popup, the checkout payment-status polling state machine,
  // the recaptcha cleanup, and the knowledge URL-id open. Under the repo's
  // `--max-warnings 0` lint gate, turning them on would force ~10 inline-disables
  // of legitimate Tier-1/Tier-2 code that catch no real bug. They remain off until
  // a surface is refactored to not need the effect, rather than annotated en masse.
  {
    files: ['apps/user/src/**/*.{ts,tsx}'],
    rules: {
      'react-hooks/immutability': 'error',
      'react-hooks/incompatible-library': 'error',
      'react-hooks/purity': 'error',
      'react-hooks/refs': 'error',
    },
  },
  prettier,
);
