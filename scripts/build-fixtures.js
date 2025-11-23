#!/usr/bin/env node
const fs = require('fs/promises');
const path = require('path');
const less = require('less');
const { compileLess } = require('../');
const { CASES } = require('./case');

const ROOT = path.join(__dirname, '..');
const FIXTURES_DIR = path.join(ROOT, 'fixtures');
const DIST_DIR = path.join(ROOT, 'dist');

async function main() {
  await fs.rm(DIST_DIR, { recursive: true, force: true });
  await fs.mkdir(DIST_DIR, { recursive: true });

  if (CASES.length === 0) {
    console.log('case.js 中未定义任何用例。');
    return;
  }

  let fallbackCount = 0;
  for (const testCase of CASES) {
    const fullPath = path.join(FIXTURES_DIR, testCase.file);
    const rel = path.relative(FIXTURES_DIR, fullPath);
    const dest = path.join(DIST_DIR, rel.replace(/\.less$/, '.css'));
    await fs.mkdir(path.dirname(dest), { recursive: true });

    const { css, engine } = await compileFixture(fullPath);
    if (engine === 'less') {
      fallbackCount += 1;
    }
    await fs.writeFile(dest, css, 'utf8');

    console.log(
      `Compiled ${rel} -> ${path.relative(ROOT, dest)} (${engine})`,
    );
  }

  console.log(`共编译 ${CASES.length} 个文件到 dist/`);
  if (fallbackCount > 0) {
    console.warn(`其中 ${fallbackCount} 个文件使用 less 回退编译。`);
  }
}

async function compileFixture(file) {
  const source = await fs.readFile(file, 'utf8');
  try {
    const css = compileLess(source, { filename: file });
    return { css, engine: 'less-oxide' };
  } catch (err) {
    console.warn(
      `less-oxide 编译失败，使用官方 less 回退: ${path.relative(
        FIXTURES_DIR,
        file,
      )}\n  ${err.message}`,
    );
    const result = await less.render(source, { filename: file, math: 'always' });
    return { css: result.css, engine: 'less' };
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
