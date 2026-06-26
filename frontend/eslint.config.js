import js from '@eslint/js';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default [
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
    languageOptions: {
      parser: tsParser,
      parserOptions: { ecmaVersion: 'latest', sourceType: 'module', ecmaFeatures: { jsx: true } },
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: {
      '@typescript-eslint': tsPlugin,
      react,
      'react-hooks': reactHooks,
    },
    settings: { react: { version: '19.2' } },
    rules: {
      ...tsPlugin.configs.recommended.rules,
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
      parser: tsParser,
      parserOptions: {
        projectService: true,
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
    },
    rules: { '@typescript-eslint/no-deprecated': 'error' },
  },
  prettier,
];
