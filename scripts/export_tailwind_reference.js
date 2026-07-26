/// 只重新导出对拍参考数据 `reference_css.json`。
///
/// `export_tailwind_classes.js` 会重跑整条抽取链路（分钟级），而调整对拍规范化逻辑时
/// 往往只需要刷新参考 CSS。两者共用 `export_tailwind/reference_css.js`，结果一致。
///
///     node scripts/export_tailwind_reference.js
///     cargo run -p silex_codegen --release && cargo fmt --all

const fs = require('fs');
const path = require('path');
const { loadDesignSystem } = require('./export_tailwind/design_system');
const { extractReferenceCss, selectReferenceCandidates } = require('./export_tailwind/reference_css');

async function main() {
  const designSystem = await loadDesignSystem();
  const dataDir = path.join(__dirname, '../crates/utils/silex_codegen/data/tailwind');

  const classList = JSON.parse(fs.readFileSync(path.join(dataDir, 'classes.json'), 'utf-8'));
  const testCases = JSON.parse(fs.readFileSync(path.join(dataDir, 'test_cases.json'), 'utf-8'));

  const candidates = selectReferenceCandidates(classList, testCases);
  const reference = extractReferenceCss(designSystem, candidates);

  const outPath = path.join(dataDir, 'reference_css.json');
  fs.writeFileSync(outPath, JSON.stringify(reference, null, 2), 'utf-8');

  console.log(`对拍参考数据已写入: ${outPath}`);
  console.log(`候选类名 ${candidates.length} 个，Tailwind 可编译 ${Object.keys(reference).length} 个`);
}

main().catch(err => {
  console.error('执行失败：', err);
  process.exit(1);
});
