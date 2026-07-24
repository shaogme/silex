const { prefixMaps } = require('./suffix_mappings');

function extractCandidates(designSystem) {
  console.log('正在提取类名与动态前缀...');
  const rawClassList = designSystem.getClassList();

  const classSet = new Set();
  const candidateDynamicPairs = [];

  function registerDynamicSuffix(prefix, suffix) {
    if (!prefix || !suffix) return;
    const prefixKey = prefix.endsWith('-') ? prefix : `${prefix}-`;
    const cls = `${prefixKey}${suffix}`;
    candidateDynamicPairs.push({ cls, prefixKey, suffix: String(suffix) });
  }

  // 1. 解析原生解析出的 class 规则
  for (const entry of rawClassList) {
    if (typeof entry === 'string') {
      classSet.add(entry);
    } else if (Array.isArray(entry)) {
      const [prefix, options] = entry;
      if (prefix) {
        classSet.add(prefix);
      }
      if (options && typeof options === 'object' && options.values) {
        let valList = [];
        if (Array.isArray(options.values)) {
          valList = options.values;
        } else if (typeof options.values === 'object') {
          valList = Object.keys(options.values);
        }
        for (const val of valList) {
          if (val === 'DEFAULT' || val === null || val === undefined) {
            if (prefix) classSet.add(prefix);
          } else if (prefix) {
            const cls = `${prefix}-${val}`;
            classSet.add(cls);
            registerDynamicSuffix(prefix, String(val));
          }
        }
      }
    }
  }

  // 2. 补充标准 Tailwind CSS 尺寸与通用后缀
  for (const { prefixes, suffixes } of prefixMaps) {
    for (const p of prefixes) {
      for (const s of suffixes) {
        if (s) {
          const cls = `${p}-${s}`;
          classSet.add(cls);
          registerDynamicSuffix(p, s);
        }
      }
    }
  }

  return {
    classSet,
    candidateDynamicPairs,
  };
}

module.exports = {
  extractCandidates,
};
