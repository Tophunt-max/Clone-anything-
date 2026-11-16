import { test } from "node:test";
import assert from "node:assert/strict";
import { loadTokens, outputs, css } from "./build.mjs";
import { readFileSync, existsSync } from "node:fs";

const t = loadTokens();

function luminance(hex) {
  const n = parseInt(hex.slice(1), 16);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

export function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

test("both themes define the same colour names", () => {
  assert.deepEqual(Object.keys(t.color.dark).sort(), Object.keys(t.color.light).sort());
});

test("every colour is a 6-digit hex", () => {
  for (const theme of ["dark", "light"]) for (const [k, v] of Object.entries(t.color[theme])) assert.match(v, /^#[0-9A-Fa-f]{6}$/, `${theme}.${k}`);
});

for (const theme of ["dark", "light"]) {
  test(`text pairs meet WCAG AA in the ${theme} theme`, () => {
    const failures = t.contrast.pairs
      .map(([fg, bg]) => [fg, bg, contrast(t.color[theme][fg], t.color[theme][bg])])
      .filter(([, , r]) => r < t.contrast.minimum)
      .map(([fg, bg, r]) => `${fg} on ${bg}: ${r.toFixed(2)}`);
    assert.deepEqual(failures, []);
  });
}

test("contrast helper matches known values", () => {
  assert.equal(contrast("#FFFFFF", "#000000").toFixed(0), "21");
  assert.equal(contrast("#777777", "#777777"), 1);
});

test("generated files are up to date", () => {
  for (const [path, text] of Object.entries(outputs(t))) {
    assert.ok(existsSync(path), `${path} missing: run the build`);
    assert.equal(readFileSync(path, "utf8"), text, `${path} is stale: run the build`);
  }
});

test("css has a variable for every colour in both themes", () => {
  const out = css(t);
  for (const k of Object.keys(t.color.dark)) assert.ok(out.includes(`--fx-${k}:`), k);
});
