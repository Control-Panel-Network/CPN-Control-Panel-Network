import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";
import tsParser from "@typescript-eslint/parser";

/**
 * ESLint 10 + eslint-config-next coordination:
 * 1. Pin settings.react.version so eslint-plugin-react skips detectReactVersion()
 *    (ESLint 10 removed context.getFilename used by that path).
 * 2. Parse JS/MJS with @typescript-eslint/parser; Next's bundled Babel parser
 *    lacks ScopeManager#addGlobals required by ESLint 10 globals.
 * See https://github.com/vercel/next.js/issues/89764
 */
const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    settings: {
      react: {
        version: "19.2.8",
      },
    },
  },
  {
    files: ["**/*.{js,mjs,cjs,jsx}"],
    languageOptions: {
      parser: tsParser,
    },
  },
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
  ]),
]);

export default eslintConfig;
