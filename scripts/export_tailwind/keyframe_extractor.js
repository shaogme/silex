function extractKeyframes(ds) {
  const keyframes = [];
  if (!ds.theme || !ds.theme.keyframes) return keyframes;

  for (const node of ds.theme.keyframes) {
    if (node.kind === 'at-rule' && node.name === '@keyframes') {
      const name = node.params;
      const steps = [];
      for (const child of node.nodes || []) {
        if (child.kind === 'rule') {
          const selector = child.selector;
          const declarations = [];
          for (const decl of child.nodes || []) {
            if (decl.kind === 'declaration') {
              declarations.push([decl.property, decl.value]);
            }
          }
          steps.push({ selector, declarations });
        }
      }
      keyframes.push({ name, steps });
    }
  }

  // 规范化特例：Tailwind v4 的 spin 仅定义了 to { transform: rotate(360deg) }
  // 为确保旋转起始状态明确，补全 from { transform: rotate(0deg) }
  for (const item of keyframes) {
    if (item.name === 'spin') {
      const hasFrom = item.steps.some(s => s.selector === 'from' || s.selector === '0%');
      if (!hasFrom) {
        item.steps.unshift({
          selector: 'from',
          declarations: [['transform', 'rotate(0deg)']]
        });
      }
    }
  }

  // 按 keyframe 名称升序排序，确保 Rust 二分查找 (binary_search_by_key) 正常有效
  keyframes.sort((a, b) => a.name.localeCompare(b.name));
  return keyframes;
}


module.exports = {
  extractKeyframes,
};
