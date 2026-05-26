import { qwikEslint9Plugin } from "eslint-plugin-qwik";
import { globalIgnores } from "eslint/config";
import tseslint from "typescript-eslint";

const ignores = [
  "**/node_modules",
  "**/dist",
  "**/server",
  "**/build",
  "**/.cache",
  "**/.wrangler",
  "**/tsconfig.tsbuildinfo",
  "**/vite.config.ts",
  "eslint.config.js",
  "worker-configuration.d.ts",
];

export default tseslint.config(globalIgnores(ignores), {
  files: ["**/*.ts", "**/*.tsx"],
  plugins: {
    qwik: qwikEslint9Plugin,
  },
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: {
      projectService: true,
      tsconfigRootDir: import.meta.dirname,
    },
  },
  rules: {
    "qwik/valid-lexical-scope": "error",
  },
});
