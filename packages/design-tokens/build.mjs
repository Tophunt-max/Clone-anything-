// Generates the platform's design tokens for every client from tokens.json.
//
//   node build.mjs          write the generated files
//   node build.mjs --check  fail if any generated file is out of date (CI)
//
// Outputs:
//   dist/tokens.css          CSS custom properties, dark (default) and light themes
//   dist/tailwind-theme.css  Tailwind v4 `@theme` mapping onto those properties
//   apps/android/.../ForgeTokens.kt  Compose constants
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const HEADER = "Generated from packages/design-tokens/tokens.json by build.mjs. Do not edit by hand.";

export function loadTokens() {
  return JSON.parse(readFileSync(join(here, "tokens.json"), "utf8"));
}

const cssVars = (colors) =>
  Object.entries(colors)
    .map(([k, v]) => `  --fx-${k}: ${v};`)
    .join("\n");

export function css(t) {
  const scale = (prefix, obj, unit = "px") =>
    Object.entries(obj)
      .filter(([k]) => !k.startsWith("$"))
      .map(([k, v]) => `  --fx-${prefix}-${k}: ${v}${unit};`)
      .join("\n");
  const shadows = Object.entries(t.elevation)
    .filter(([k]) => !k.startsWith("$"))
    .map(([k, v]) => `  --fx-shadow-${k}: ${v.shadow};`)
    .join("\n");
  return `/* ${HEADER} */
:root,
[data-theme="dark"] {
  color-scheme: dark;
${cssVars(t.color.dark)}
}

[data-theme="light"] {
  color-scheme: light;
${cssVars(t.color.light)}
}

@media (prefers-color-scheme: light) {
  :root:not([data-theme]) {
    color-scheme: light;
${cssVars(t.color.light).replace(/^ {2}/gm, "    ")}
  }
}

:root {
  --fx-font-sans: ${t.font.sans};
  --fx-font-mono: ${t.font.mono};
  --fx-line-height: ${t.lineHeight};
${scale("text", t.fontSize)}
${scale("space", t.space)}
${scale("radius", t.radius)}
${shadows}
}
`;
}

export function tailwindTheme(t) {
  const colors = Object.keys(t.color.dark)
    .map((k) => `  --color-${k}: var(--fx-${k});`)
    .join("\n");
  const sizes = Object.entries(t.fontSize)
    .map(([k]) => `  --text-${k}: var(--fx-text-${k});`)
    .join("\n");
  const radii = Object.keys(t.radius)
    .map((k) => `  --radius-${k}: var(--fx-radius-${k});`)
    .join("\n");
  const shadows = Object.keys(t.elevation)
    .filter((k) => !k.startsWith("$"))
    .map((k) => `  --shadow-e${k}: var(--fx-shadow-${k});`)
    .join("\n");
  return `/* ${HEADER} */
/* Import after "tailwindcss" and tokens.css. Utilities: bg-bg-canvas, text-text-primary, rounded-md, shadow-e1… */
@theme inline {
  --font-sans: var(--fx-font-sans);
  --font-mono: var(--fx-font-mono);
  --breakpoint-md: ${t.breakpoint.md}px;
  --breakpoint-lg: ${t.breakpoint.lg}px;
${colors}
${sizes}
${radii}
${shadows}
}
`;
}

const camel = (k) => k.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
const argb = (hex) => `0xFF${hex.replace("#", "").toUpperCase()}`;

export function kotlin(t) {
  const palette = (name, colors) =>
    `    object ${name} {\n` +
    Object.entries(colors)
      .map(([k, v]) => `        val ${camel(k)} = Color(${argb(v)})`)
      .join("\n") +
    "\n    }";
  const dims = (name, obj, unit) =>
    `    object ${name} {\n` +
    Object.entries(obj)
      .filter(([k]) => !k.startsWith("$"))
      .map(([k, v]) => `        val ${camel(k)} = ${v}.${unit}`)
      .join("\n") +
    "\n    }";
  const elevation =
    "    object Elevation {\n" +
    Object.entries(t.elevation)
      .filter(([k]) => !k.startsWith("$"))
      .map(([k, v]) => `        val level${k} = ${v.dp}.dp`)
      .join("\n") +
    "\n    }";
  return `// ${HEADER}
package com.tophunt.forge.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

object ForgeTokens {
${palette("Dark", t.color.dark)}

${palette("Light", t.color.light)}

${dims("FontSize", t.fontSize, "sp")}

${dims("Space", t.space, "dp")}

${dims("Radius", t.radius, "dp")}

${elevation}

    const val LINE_HEIGHT = ${t.lineHeight}f
    const val BREAKPOINT_MEDIUM_DP = ${t.breakpoint.md}
    const val BREAKPOINT_LARGE_DP = ${t.breakpoint.lg}
}
`;
}

export function outputs(t) {
  return {
    [join(here, "dist", "tokens.css")]: css(t),
    [join(here, "dist", "tailwind-theme.css")]: tailwindTheme(t),
    [join(root, "apps", "android", "app", "src", "main", "java", "com", "tophunt", "forge", "ui", "theme", "ForgeTokens.kt")]: kotlin(t),
  };
}

function main() {
  const check = process.argv.includes("--check");
  const stale = [];
  for (const [path, text] of Object.entries(outputs(loadTokens()))) {
    if (check) {
      if (!existsSync(path) || readFileSync(path, "utf8") !== text) stale.push(relative(root, path));
    } else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, text);
    }
  }
  if (stale.length) {
    console.error(`design-tokens: out of date, run \`pnpm --filter @forge/design-tokens build\`:\n  ${stale.join("\n  ")}`);
    process.exit(1);
  }
  console.log(check ? "design-tokens: up to date" : "design-tokens: written");
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
