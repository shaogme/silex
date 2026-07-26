/// Tailwind 输出 CSS 的通用解析辅助。
///
/// 这些函数原本内联在 `metadata_probe.js` 中，`reference_css.js`（对拍测试参考数据导出）
/// 需要完全一致的剥离/解析语义，故抽出共用，避免两份实现再次漂移。

/// 永远不应出现在 target_props 中的 at-rule 描述符（`@property` 块的成员）
const DESCRIPTOR_PROPS = new Set(['syntax', 'inherits', 'initial-value']);

/// 剥离 `@property {...}` / `@keyframes {...}` 等描述符声明块。
///
/// 这些 at-rule 的内部声明（syntax/inherits/initial-value、动画关键帧）不是工具类的目标属性，
/// 直接对整段 CSS 做裸正则匹配会把它们误当作 target_props
/// （旧实现即因此污染了 border-s/e/bs/be，见分析报告 §2.3）。
function stripDescriptorAtRules(css) {
  let out = '';
  let i = 0;
  while (i < css.length) {
    const at = css.indexOf('@', i);
    if (at === -1) {
      out += css.slice(i);
      break;
    }
    const kwMatch = /^@(property|keyframes|font-face|counter-style)\b/.exec(css.slice(at));
    if (!kwMatch) {
      out += css.slice(i, at + 1);
      i = at + 1;
      continue;
    }
    const open = css.indexOf('{', at);
    if (open === -1) {
      out += css.slice(i);
      break;
    }
    // 平衡括号扫描，跳过整个块
    let depth = 0;
    let j = open;
    for (; j < css.length; j++) {
      if (css[j] === '{') depth++;
      else if (css[j] === '}') {
        depth--;
        if (depth === 0) {
          j++;
          break;
        }
      }
    }
    out += css.slice(i, at);
    i = j;
  }
  return out;
}

/// 匹配一条声明：`prop: value;`（value 中不含 `;{}`）
const DECL_RE = /(--[a-zA-Z0-9_-]+|[a-zA-Z-][a-zA-Z0-9_-]*)\s*:\s*([^;{}]+);/g;

/// 解析一段工具类 CSS 为 `[[prop, value], ...]`（已剥离描述符 at-rule）。
///
/// 保留出现顺序与重复项，调用方按需去重——`metadata_probe` 需要 first-wins 的 Map，
/// 而对拍导出需要完整的声明序列（例如 `container` 的多条 max-width）。
function parseDeclarationList(cssStr) {
  const cleaned = stripDescriptorAtRules(cssStr);
  const out = [];
  for (const m of cleaned.matchAll(DECL_RE)) {
    const prop = m[1];
    if (DESCRIPTOR_PROPS.has(prop)) continue;
    out.push([prop, m[2].trim()]);
  }
  return out;
}

module.exports = {
  DESCRIPTOR_PROPS,
  stripDescriptorAtRules,
  parseDeclarationList,
};
