function extractProperties(classList, designSystem, batchSize = 3000) {
  console.log('正在从编译生成的 CSS 中提取自定义变量与特殊属性...');
  const extraPropsSet = new Set();

  for (let i = 0; i < classList.length; i += batchSize) {
    const batch = classList.slice(i, i + batchSize);
    const cssList = designSystem.candidatesToCss(batch);
    for (const cssStr of cssList) {
      if (!cssStr || typeof cssStr !== 'string') continue;
      const matches = Array.from(cssStr.matchAll(/([a-zA-Z0-9_\-]+)\s*:/g));
      for (const match of matches) {
        const propName = match[1];
        if (propName.startsWith('--') || propName.startsWith('-')) {
          extraPropsSet.add(propName);
        }
      }
    }
  }

  // 确保 border 与 transform 等基础 Key 在 extra_properties 中存在
  extraPropsSet.add('border');
  extraPropsSet.add('transform');

  const result = Array.from(extraPropsSet).sort();
  console.log(`已自动提取出 ${result.length} 个自定义变量与特殊前缀属性。`);
  return result;
}

function extractPropertyAliases(classList, designSystem) {
  console.log('正在自动推导简写 CSS 属性与原子子属性 (Property Aliases) 映射表...');
  const aliasesObj = {};

  // 通用物理维度与方向后缀推导规则
  const dirRules = [
    { suffix: '-inline', replacements: ['-left', '-right'] },
    { suffix: '-block', replacements: ['-top', '-bottom'] },
    { suffix: '-x', replacements: ['-left', '-right'] },
    { suffix: '-y', replacements: ['-top', '-bottom'] },
  ];

  const basePrefixes = [
    'padding', 'margin', 'scroll-padding', 'scroll-margin',
    'border', 'border-width', 'border-style', 'border-color'
  ];

  for (const prefix of basePrefixes) {
    for (const rule of dirRules) {
      let key = `${prefix}${rule.suffix}`;
      // 处理 border-x-width, border-y-color 等形式
      if (prefix.startsWith('border-') && prefix !== 'border') {
        const subParts = prefix.split('-'); // e.g. ["border", "width"]
        key = `border${rule.suffix}-${subParts[1]}`;
      }

      let subProps;
      if (prefix.startsWith('border-') && prefix !== 'border') {
        const parts = prefix.split('-');
        subProps = rule.replacements.map(r => `border${r}-${parts[1]}`);
      } else {
        subProps = rule.replacements.map(r => `${prefix}${r}`);
      }

      aliasesObj[key] = subProps;
    }
  }

  // inset 相关别名
  aliasesObj['inset'] = ['top', 'right', 'bottom', 'left'];
  aliasesObj['inset-x'] = ['left', 'right'];
  aliasesObj['inset-y'] = ['top', 'bottom'];

  // border 基础全属性映射
  aliasesObj['border'] = [
    'border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width',
    'border-top-style', 'border-right-style', 'border-bottom-style', 'border-left-style',
    'border-top-color', 'border-right-color', 'border-bottom-color', 'border-left-color'
  ];

  // 对所有的 key 和 subProps 进行排序与规范化
  const sortedKeys = Object.keys(aliasesObj).sort();
  const sortedResult = {};
  for (const k of sortedKeys) {
    sortedResult[k] = aliasesObj[k];
  }

  console.log(`已自动推导出 ${Object.keys(sortedResult).length} 组简写属性映射。`);
  return sortedResult;
}

module.exports = {
  extractProperties,
  extractPropertyAliases,
};

