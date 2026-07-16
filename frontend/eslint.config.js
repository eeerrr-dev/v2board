import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import pluginQuery from '@tanstack/eslint-plugin-query';

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
  ...pluginQuery.configs['flat/recommended'],
  {
    files: ['**/*.config.mjs', 'scripts/**/*.mjs', 'tests/**/*.mjs'],
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
      'no-empty': 'error',
      'no-undef': 'off',
      'no-unused-expressions': ['error', { allowShortCircuit: true, allowTernary: true }],
      'react/jsx-key': 'error',
      'react/no-unknown-property': ['error', { ignore: ['unselectable'] }],
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',
      'react/jsx-no-target-blank': 'error',
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
  // The only deliberately retained browser deprecation is BeforeUnloadEvent.returnValue,
  // which remains necessary for the unsaved server-order prompt. It carries a local,
  // documented lint suppression; deprecated convenience APIs and compatibility shims
  // are otherwise rejected across both redesigned apps.
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
    rules: {
      '@typescript-eslint/no-deprecated': 'error',
      // Async-safety set (same type-aware scope). Dropped promises silently
      // swallow rejections; intentional detachment must be spelled `void p`.
      '@typescript-eslint/await-thenable': 'error',
      '@typescript-eslint/no-floating-promises': 'error',
      // Async JSX handlers (onClick etc.) are the standard React idiom, so
      // attribute positions are exempt from the void-return check.
      '@typescript-eslint/no-misused-promises': [
        'error',
        { checksVoidReturn: { attributes: false } },
      ],
    },
  },
  // Both redesigned apps run React Compiler. Keep its correctness diagnostics
  // enabled for every authored component so compilation never silently bails
  // out behind a globally disabled lint rule.
  //
  // Enforced: purity / refs / immutability / incompatible-library — these are the
  // rules the compiler actually relies on to safely auto-memoize. They are clean
  // across the user app, except TanStack Table's useReactTable (an inherently
  // non-memoizable API), disabled inline at that one call site.
  //
  // Source hooks use the full correctness set. State that mirrors queries/props is
  // modeled as derived or keyed state, and effects are reserved for external
  // synchronization (timers, subscriptions, focus and imperative libraries).
  {
    files: ['apps/*/src/**/*.{ts,tsx}'],
    rules: {
      'react-hooks/immutability': 'error',
      'react-hooks/incompatible-library': 'error',
      'react-hooks/purity': 'error',
      'react-hooks/refs': 'error',
      'react-hooks/exhaustive-deps': 'error',
      'react-hooks/set-state-in-effect': 'error',
    },
  },
  prettier,
);
