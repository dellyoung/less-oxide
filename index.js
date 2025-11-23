'use strict';

// 载入 napi 构建的原生模块，兼容不同文件名。
let nativeBinding;
try {
  nativeBinding = require('./less_oxide.node');
} catch (err) {
  try {
    nativeBinding = require('./index.node');
  } catch (inner) {
    throw new Error(
      '未找到 less_oxide.node/index.node，请先执行 npm run build。' +
        ` 原始错误: ${inner.message}`
    );
  }
}

/**
 * 编译 LESS 字符串为 CSS。
 * @param {string} source LESS 源码
 * @param {{ minify?: boolean }} [options] 编译配置
 * @returns {string} CSS 结果
 */
function compileLess(source, options = {}) {
  if (typeof source !== 'string') {
    throw new TypeError('source 必须是字符串');
  }
  return nativeBinding.compileLess(source, options);
}

module.exports = {
  compileLess,
  compile: compileLess,
};
