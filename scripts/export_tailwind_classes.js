const fs = require('fs');
const path = require('path');
const { loadDesignSystem } = require('./export_tailwind/design_system');
const { extractCandidates } = require('./export_tailwind/candidate_extractor');
const { filterValidClasses } = require('./export_tailwind/validator');
const { inferPrefixMetadata } = require('./export_tailwind/metadata_probe');
const { extractPalette } = require('./export_tailwind/palette_extractor');
const { extractModifiers } = require('./export_tailwind/modifier_extractor');
const { extractKeyframes } = require('./export_tailwind/keyframe_extractor');

async function main() {
  // 1. 加载设计系统
  const designSystem = await loadDesignSystem();

  // 2. 提取标准色板 (palette)
  const paletteObj = extractPalette(designSystem);

  // 3. 提取修饰符元数据 (modifiers)
  const modifiersList = extractModifiers(designSystem);

  // 4. 提取动画 Keyframes (keyframes)
  const keyframesList = extractKeyframes(designSystem);

  // 4. 提取候选类名与动态前缀-后缀对
  const { classSet, candidateDynamicPairs } = extractCandidates(designSystem);

  // 5. 二次过滤，剔除合成了但 Tailwind 无法编译的非法类名（按每批 3000 个分批）
  const filteredClassSet = await filterValidClasses(classSet, designSystem, 3000);

  // 6. 清理并排序类名列表
  const classList = Array.from(filteredClassSet)
    .filter(cls => typeof cls === 'string' && cls.trim().length > 0)
    .filter(cls => !cls.startsWith('space-') && !cls.startsWith('-space-') && !cls.startsWith('divide-') && !cls.startsWith('-divide-'))
    .sort();

  // 7. 提取与验证动态前缀及测试用例
  const validDynamicPrefixesMap = new Map();
  for (const { cls, prefixKey, suffix } of candidateDynamicPairs) {
    if (filteredClassSet.has(cls)) {
      if (!validDynamicPrefixesMap.has(prefixKey)) {
        validDynamicPrefixesMap.set(prefixKey, new Set());
      }
      validDynamicPrefixesMap.get(prefixKey).add(suffix);
    }
  }

  const dynamicPrefixesObj = {};
  const sortedPrefixKeys = Array.from(validDynamicPrefixesMap.keys()).sort();
  const testCasesSet = new Set();

  for (const key of sortedPrefixKeys) {
    const suffixes = Array.from(validDynamicPrefixesMap.get(key)).sort();
    if (suffixes.length > 0) {
      dynamicPrefixesObj[key] = suffixes;

      // 选取多种代表性后缀用于验证表
      const sampleIndices = new Set();
      sampleIndices.add(0); // 头部后缀
      sampleIndices.add(Math.floor(suffixes.length / 2)); // 中间后缀
      sampleIndices.add(suffixes.length - 1); // 尾部后缀

      // 额外的常用代表性后缀（如果存在）
      ['0', '1', '4', 'full', 'auto', 'px', 'sm', 'md', 'lg', 'xl', '50', '100', 'none', 'DEFAULT'].forEach(s => {
        const idx = suffixes.indexOf(s);
        if (idx !== -1) sampleIndices.add(idx);
      });

      for (const idx of sampleIndices) {
        const suf = suffixes[idx];
        const testCls = `${key}${suf}`;
        if (filteredClassSet.has(testCls)) {
          testCasesSet.add(testCls);
        }
      }
    }
  }

  const testCasesList = Array.from(testCasesSet).sort();

  // 8. 探针分析并自动推导 Dynamic Prefix 元数据 (prefix_metadata)
  const inferredMetadataObj = inferPrefixMetadata(sortedPrefixKeys, designSystem);

  // 9. 导出 JSON 数据到 crates/utils/silex_codegen/data/tailwind 目录
  const outputDir = path.join(__dirname, '../crates/utils/silex_codegen/data/tailwind');
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  fs.writeFileSync(path.join(outputDir, 'classes.json'), JSON.stringify(classList, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'dynamic_prefixes.json'), JSON.stringify(dynamicPrefixesObj, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'prefix_metadata.json'), JSON.stringify(inferredMetadataObj, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'test_cases.json'), JSON.stringify(testCasesList, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'palette.json'), JSON.stringify(paletteObj, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'modifiers.json'), JSON.stringify(modifiersList, null, 2), 'utf-8');
  fs.writeFileSync(path.join(outputDir, 'keyframes.json'), JSON.stringify(keyframesList, null, 2), 'utf-8');

  console.log(`导出成功！文件已保存至: ${outputDir}`);
  console.log(`共包含类名数量：${classList.length}`);
  console.log(`包含动态前缀数量：${Object.keys(dynamicPrefixesObj).length}`);
  console.log(`包含前缀元数据数量：${Object.keys(inferredMetadataObj).length}`);
  console.log(`包含验证测试用例数量：${testCasesList.length}`);
  console.log(`包含标准色板色系数量：${Object.keys(paletteObj).length}`);
  console.log(`包含修饰符元数据数量：${modifiersList.length}`);
  console.log(`包含动画 Keyframes 数量：${keyframesList.length}`);
}

main().catch(err => {
  console.error('执行失败：', err);
  process.exit(1);
});