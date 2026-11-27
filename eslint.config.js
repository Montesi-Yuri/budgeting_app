import js from '@eslint/js'
import stylistic from '@stylistic/eslint-plugin'
import reactHooks from 'eslint-plugin-react-hooks'
import { defineConfig, globalIgnores } from 'eslint/config'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default defineConfig([
	globalIgnores(['dist', 'target', 'src-tauri', 'core', '.scratch']),
	{
		files: ['**/*.{js,mjs,ts,tsx}'],
		extends: [
			js.configs.recommended,
			tseslint.configs.recommended,
			reactHooks.configs.flat.recommended,
			stylistic.configs.customize({
				quotes: 'single',
				semi: false,
				braceStyle: '1tbs',
				arrowParens: true,
				indent: 'tab',
			}),
		],
		languageOptions: {
			globals: { ...globals.browser, ...globals.node },
		},
	},
])
