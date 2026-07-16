import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import eslintReact from '@eslint-react/eslint-plugin';
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
    // @eslint-react's recommended-typescript preset is the React 19-era
    // replacement for eslint-plugin-react: JSX/DOM misuse rules without the
    // prop-types/react-in-scope legacy, TypeScript-aware by design.
    extends: [tseslint.configs.recommended, eslintReact.configs['recommended-typescript']],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaVersion: 'latest', sourceType: 'module', ecmaFeatures: { jsx: true } },
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: {
      'react-hooks': reactHooks,
    },
    rules: {
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
      // eslint-plugin-react-hooks v7 ships the React Compiler's own
      // diagnostics and owns the hooks/compiler domain here; disable
      // @eslint-react's overlapping implementations so each violation
      // reports once, from the authoritative plugin.
      '@eslint-react/error-boundaries': 'off',
      '@eslint-react/exhaustive-deps': 'off',
      '@eslint-react/purity': 'off',
      '@eslint-react/rules-of-hooks': 'off',
      '@eslint-react/set-state-in-effect': 'off',
      '@eslint-react/static-components': 'off',
      '@eslint-react/unsupported-syntax': 'off',
      '@eslint-react/use-memo': 'off',
      // Not in the preset (TypeScript already rejects unknown JSX props);
      // kept for the security outcome the old jsx-no-target-blank enforced.
      '@eslint-react/dom-no-unsafe-target-blank': 'error',
      // Every current index-key site is legitimate: static option/nav lists,
      // options whose submitted value IS the index, id-less append-only chat
      // messages, and a child-key fallback. The heuristic only adds noise here.
      '@eslint-react/no-array-index-key': 'off',
      // Existing refs carry domain names (saveTail, queuedSaves, mounted) that
      // read better than a mechanical *Ref suffix.
      '@eslint-react/naming-convention-ref-name': 'off',
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
