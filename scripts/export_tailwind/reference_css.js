/// 对拍测试（differential testing）参考数据导出。
///
/// 分析报告 §6.3.2：仓库内已有真实的 `tailwindcss` 依赖，可以直接用它作为**语义真值**，
/// 与 `tw!` 宏的产出逐条对照。这是拦截语义漂移（ring / blur / outline-none / container …）
/// 投入产出比最高的一道防线。
///
/// 本模块负责把 Tailwind 自己编译出来的 CSS 规范化到"可与 silex 产物比较"的形式：
///
/// 1. 剥离 `@property` 等描述符块（silex 不产出它们，属于全局前导）；
/// 2. 把 theme 变量展开为字面量——Tailwind 产出 `padding: calc(var(--spacing) * 4)`，
///    而 silex 直接解析为 `padding: 1rem`，不展开就没有可比性；
/// 3. 求值简单 `calc()`；
/// 4. oklch 颜色转 hex（与 `palette_extractor.js` 用同一套转换，保证与 palette.json 一致）。
///
/// `--tw-*` 这类**运行时**变量不属于 theme，保持原样——它们本身就是被比较的对象。

const { parseDeclarationList } = require('./css_utils');

/// 从设计系统里取出全部 theme 变量
function buildThemeMap(designSystem) {
  const map = new Map();
  for (const [key, entry] of designSystem.theme.entries()) {
    if (!entry || typeof entry.value !== 'string') continue;
    let value = entry.value.replace(/\s+/g, ' ').trim();
    map.set(key, value);
  }
  return map;
}

/// 递归展开 `var(--theme-token)`；未登记在 theme 中的变量（`--tw-*` 等）保持原样。
///
/// 带兜底值的 `var(--x, fallback)`：若 `--x` 是 theme 变量则取其值，否则整体保留，
/// 因为兜底分支的取舍是运行时行为，编译期无法断言。
function expandThemeVars(value, theme, depth = 0) {
  if (depth > 8 || !value.includes('var(')) return value;

  let out = '';
  let i = 0;
  while (i < value.length) {
    const start = value.indexOf('var(', i);
    if (start === -1) {
      out += value.slice(i);
      break;
    }
    // 平衡括号找到 var(...) 的收尾
    let depthParen = 0;
    let j = start + 3;
    for (; j < value.length; j++) {
      if (value[j] === '(') depthParen++;
      else if (value[j] === ')') {
        depthParen--;
        if (depthParen === 0) break;
      }
    }
    if (j >= value.length) {
      out += value.slice(i);
      break;
    }
    const inner = value.slice(start + 4, j);
    const commaAt = inner.indexOf(',');
    const name = (commaAt === -1 ? inner : inner.slice(0, commaAt)).trim();

    out += value.slice(i, start);
    if (theme.has(name)) {
      out += expandThemeVars(theme.get(name), theme, depth + 1);
    } else {
      out += value.slice(start, j + 1);
    }
    i = j + 1;
  }
  return out;
}

const NUM_UNIT_RE = /^(-?[\d.]+)([a-z%]*)$/i;

/// 求值形如 `calc(A * B)` / `calc(A / B)` 的简单表达式（Tailwind 的间距体系只用到这两种）。
/// 无法求值时原样返回。
function evalSimpleCalc(value, depth = 0) {
  if (depth > 6 || !value.includes('calc(')) return value;

  let out = '';
  let i = 0;
  while (i < value.length) {
    const start = value.indexOf('calc(', i);
    if (start === -1) {
      out += value.slice(i);
      break;
    }
    let depthParen = 0;
    let j = start + 4;
    for (; j < value.length; j++) {
      if (value[j] === '(') depthParen++;
      else if (value[j] === ')') {
        depthParen--;
        if (depthParen === 0) break;
      }
    }
    if (j >= value.length) {
      out += value.slice(i);
      break;
    }
    const inner = evalSimpleCalc(value.slice(start + 5, j), depth + 1).trim();
    out += value.slice(i, start);
    out += reduceProduct(inner) ?? `calc(${inner})`;
    i = j + 1;
  }
  return out;
}

/// `0.25rem * 4` → `1rem`；带单位的操作数至多一个，否则返回 null
function reduceProduct(expr) {
  // Tailwind 的分数工具类会产出 `2/4 * 100%` 这种混合了紧凑与松散写法的表达式，
  // 先把运算符两侧补齐空格再切分
  const parts = expr.replace(/([*/])/g, ' $1 ').split(/\s+([*/])\s+/);
  if (parts.length < 3 || parts.length % 2 === 0) return null;

  let acc = NUM_UNIT_RE.exec(parts[0]);
  if (!acc) return null;
  let num = parseFloat(acc[1]);
  let unit = acc[2];

  for (let k = 1; k < parts.length; k += 2) {
    const op = parts[k];
    const operand = NUM_UNIT_RE.exec(parts[k + 1]);
    if (!operand) return null;
    const opNum = parseFloat(operand[1]);
    const opUnit = operand[2];
    if (unit && opUnit) return null; // 两个带单位的量相乘/相除，语义超出本求值器
    if (op === '*') {
      num *= opNum;
      unit = unit || opUnit;
    } else {
      if (opNum === 0) return null;
      num /= opNum;
      if (!unit) unit = opUnit;
    }
  }

  // 去掉浮点误差尾巴（0.30000000000000004 → 0.3）
  const rounded = Math.round(num * 1e6) / 1e6;
  return `${rounded}${unit}`;
}

function normalizeValue(raw, theme) {
  let v = expandThemeVars(raw, theme);
  v = evalSimpleCalc(v);
  return v.replace(/\s+/g, ' ').trim();
}

/// 为一批候选类名导出 Tailwind 的参考声明。
///
/// 返回 `{ [candidate]: [[prop, value], ...] }`；Tailwind 无法编译的候选被跳过
/// （不会出现在结果里，调用方据此得知该类名不是合法 Tailwind 类）。
function extractReferenceCss(designSystem, candidates) {
  const theme = buildThemeMap(designSystem);
  const result = {};

  // candidatesToCss 对大批量输入很慢，分批处理
  const BATCH = 500;
  for (let start = 0; start < candidates.length; start += BATCH) {
    const chunk = candidates.slice(start, start + BATCH);
    const cssList = designSystem.candidatesToCss(chunk);
    chunk.forEach((candidate, idx) => {
      const css = cssList[idx];
      if (!css || typeof css !== 'string' || css.trim().length === 0) return;

      const decls = parseDeclarationList(css)
        .map(([prop, value]) => [prop, normalizeValue(value, theme)])
        .filter(([, value]) => value.length > 0);

      if (decls.length > 0) {
        result[candidate] = decls;
      }
    });
  }

  return result;
}

/// 对拍必须覆盖的"热点"类名——历史上出过静默错误、或语义容易漂移的地方。
/// 这些即便不在分层抽样里也一定入选。
const HOTSPOT_CANDIDATES = [
  // §2.4 ring 体系
  'ring', 'ring-0', 'ring-1', 'ring-2', 'ring-4', 'ring-blue-500', 'ring-inset',
  'ring-offset-2', 'ring-offset-blue-500', 'inset-ring-2', 'inset-ring-red-500',
  // §2.2 filter 家族
  'blur-sm', 'blur-[4px]', 'backdrop-blur-[4px]', 'brightness-[1.75]',
  'brightness-50', 'contrast-125', 'grayscale', 'hue-rotate-90', 'invert',
  'saturate-150', 'sepia', 'drop-shadow-lg',
  // §2.3 逻辑属性方向类的任意值路径
  'border-s-[3px]', 'border-e-[3px]', 'border-bs-[3px]', 'border-be-[3px]',
  'border-s-2', 'border-e-2', 'ms-4', 'me-4', 'ps-4', 'pe-4',
  // §2.10 outline / object-fit / skew 曾产出非法属性
  'outline-none', 'outline-hidden', 'outline-2', 'outline-red-500',
  'object-fill', 'object-cover', 'skew-x-6', 'skew-y-6',
  // 颜色 + 不透明度（§2.6）
  'bg-red-500', 'bg-red-500/50', 'text-red-500', 'border-red-500',
  // 任意值与多义前缀（§2.8）
  'p-[10px]', 'w-[50%]', 'text-[14px]', 'bg-[#123456]', 'grid-cols-[1fr_2fr]',
  // 变换与间距
  'translate-x-2', 'translate-y-2', 'rotate-45', 'scale-95',
  'space-x-4', 'divide-x-2', 'gap-4', 'gap-x-4',
  // 渐变
  'bg-linear-to-r', 'from-red-500', 'via-blue-500', 'to-green-500',
  // 容器与排版
  'container', 'leading-6', 'tracking-tight', 'line-clamp-3',
  // 阴影与动画
  'shadow-lg', 'shadow-none', 'animate-spin', 'duration-300', 'delay-150', 'ease-in-out',
];

/// 挑选参与对拍的候选类名。
///
/// 全量 22 879 条类名会让夹具膨胀到几十 MB，而只用 `test_cases.json` 又覆盖不到冷门家族。
/// 折中方案：热点清单 + 全部 test_cases + 按前缀家族分层等距抽样，保证每个家族都有代表，
/// 且结果只依赖输入顺序（可重现，不引入随机性）。
function selectReferenceCandidates(classList, testCases, perFamily = 6) {
  const selected = new Set();
  for (const c of HOTSPOT_CANDIDATES) selected.add(c);
  for (const c of testCases) selected.add(c);

  const families = new Map();
  for (const cls of classList) {
    const family = cls.replace(/^-/, '').split('-')[0];
    if (!families.has(family)) families.set(family, []);
    families.get(family).push(cls);
  }

  for (const members of families.values()) {
    const step = Math.max(1, Math.floor(members.length / perFamily));
    for (let i = 0; i < members.length && i / step < perFamily; i += step) {
      selected.add(members[i]);
    }
  }

  return Array.from(selected).sort();
}

module.exports = {
  extractReferenceCss,
  selectReferenceCandidates,
  HOTSPOT_CANDIDATES,
  buildThemeMap,
  expandThemeVars,
  evalSimpleCalc,
};
