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

  const result = Array.from(extraPropsSet).sort();
  console.log(`已自动提取出 ${result.length} 个自定义变量与特殊前缀属性。`);
  return result;
}

module.exports = {
  extractProperties,
};
