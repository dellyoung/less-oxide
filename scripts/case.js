#!/usr/bin/env node
const path = require('path');

const CASES = [
  { name: 'baseline', file: 'benchmark.less' },
  { name: 'import', file: 'import.less' },
  { name: 'mixins', file: 'mixins.less' },
  { name: 'arithmetic', file: 'arithmetic.less' },
  { name: 'at-rules', file: 'at-rules.less' },
  { name: 'styles-base', file: path.join('styles', 'base.less'), verify: false },
];

module.exports = { CASES };
