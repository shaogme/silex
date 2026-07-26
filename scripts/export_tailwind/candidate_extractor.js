const { prefixMaps } = require('./suffix_mappings');

/// `getClassList()` 之外，功能型前缀还接受的"裸关键字"后缀。
///
/// `getClassList()` 只枚举有 theme 命名空间可展开的取值，`filter-none` 这种
/// 值不来自 theme 的档位因此整条缺席。这里对**全部**功能型前缀做一遍关键字扫描，
/// 能不能编译交给 `filterValidClasses`（真实 Tailwind）判定——比手写一张
/// "还缺哪些类名"的补丁表可靠，上游新增同形状的工具类也会自动被带进来。
const bareKeywordSuffixes = ['none', 'auto', 'full', 'normal', 'initial', 'px', 'screen', 'fit', 'min', 'max'];

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

  // 2. 补齐 `getClassList()` 遗漏的工具类
  //
  // `getClassList()` 是给编辑器补全用的，只覆盖"能从 theme 展开出取值"的部分：
  // 静态工具类里的一批 v3 兼容别名（`bg-left-top` / `overflow-ellipsis` / `break-words` …）
  // 与取值不来自 theme 的功能型工具类（`filter-none` / `backdrop-filter-none`）整条缺席。
  // `utilities.keys()` 才是 Tailwind 自己的注册表，用它当真值源。
  if (designSystem.utilities && typeof designSystem.utilities.keys === 'function') {
    for (const cls of designSystem.utilities.keys('static')) {
      classSet.add(cls);
    }
    for (const prefix of designSystem.utilities.keys('functional')) {
      // 负号形式（`-mt`）由 resolver 的取负路径处理，不进静态表
      if (prefix.startsWith('-')) continue;
      for (const suffix of bareKeywordSuffixes) {
        // 只补类名，**不**调用 `registerDynamicSuffix`：
        // 「`cursor-auto` 能编译」并不足以推断 `cursor-` 是一个接受任意值的动态前缀，
        // 而 `dynamic_prefixes` 一旦登记，探针推导出的 `target_props` / `unit_kind`
        // 就会成为 `cursor-[…]` 这类任意值的求值依据——按这条弱证据放进去，
        // 等于让 98 个前缀的任意值路径一次性上线且无人验证。
        classSet.add(`${prefix}-${suffix}`);
      }
    }
  }

  // 3. 补充标准 Tailwind CSS 尺寸与通用后缀
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
