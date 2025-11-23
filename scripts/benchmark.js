#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const { performance } = require('perf_hooks');
const less = require('less');
const { compileLess } = require('../');
const { CASES } = require('./case');

const ITERATIONS = Number(process.argv[2] || '200');
const FIXTURE_DIR = path.join(__dirname, '..', 'fixtures');

const MODES = [
  { label: 'pretty', minify: false },
  { label: 'minified', minify: true },
];

async function timeAsync(fn, iterations) {
  const start = performance.now();
  for (let i = 0; i < iterations; i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await fn();
  }
  return performance.now() - start;
}

async function main() {
  console.log(`Benchmark iterations: ${ITERATIONS}`);

  const summary = [];

  for (const testCase of CASES) {
    const fullPath = path.join(FIXTURE_DIR, testCase.file);
    const source = fs.readFileSync(fullPath, 'utf8');

    for (const mode of MODES) {
      const label = `${testCase.name}-${mode.label}`;
      const renderOptions = { compress: mode.minify, math: 'always', filename: fullPath };
      const rustFn = () => compileLess(source, { minify: mode.minify, filename: fullPath });
      const lessFn = () => less.render(source, renderOptions).then((res) => res.css);

      const rustCss = rustFn();
      const lessCss = await lessFn();

      if (testCase.verify !== false && !compareOutputs(rustCss, lessCss)) {
        throw new Error(`输出不一致: ${label}`);
      }

      const rustDuration = await timeAsync(() => ensurePromise(rustFn()), ITERATIONS);
      const lessDuration = await timeAsync(() => ensurePromise(lessFn()), ITERATIONS);

      summary.push({
        label,
        rust: rustDuration / ITERATIONS,
        less: lessDuration / ITERATIONS,
      });
    }
  }

  console.log('\nCase                        less-oxide (ms)   less (ms)   Speedup');
  console.log('----------------------------------------------------------------');
  for (const row of summary) {
    const speedup = row.less / row.rust;
    console.log(
      `${row.label.padEnd(26)} ${row.rust.toFixed(3).padStart(14)} ${row.less
        .toFixed(3)
        .padStart(11)} ${speedup.toFixed(2).padStart(9)}x`,
    );
  }
}

function compareOutputs(a, b) {
  return normalizeCss(a) === normalizeCss(b);
}

function normalizeCss(css) {
  return css
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\s+/g, ' ')
    .replace(/\s*([{}:;,])\s*/g, '$1')
    .replace(/\b0+(\.\d+)/g, '$1')
    .trim();
}

function ensurePromise(result) {
  if (result && typeof result.then === 'function') {
    return result;
  }
  return Promise.resolve(result);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
