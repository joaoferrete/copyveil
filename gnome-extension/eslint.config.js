import js from '@eslint/js';

export default [
    js.configs.recommended,
    {
        files: ['**/*.js'],
        languageOptions: {
            ecmaVersion: 2024,
            sourceType: 'module',
            globals: {
                global: 'readonly',
                log: 'readonly',
                logError: 'readonly',
                console: 'readonly',
                // Present in both GJS and Node, which runs the lib/ tests.
                TextDecoder: 'readonly',
                TextEncoder: 'readonly',
            },
        },
        rules: {
            // Item content must never reach the journal.
            'no-console': 'error',
            'no-restricted-globals': ['error', 'log', 'logError'],
            'no-unused-vars': ['error', {argsIgnorePattern: '^_'}],
            'prefer-const': 'error',
            eqeqeq: 'error',
        },
    },
];
