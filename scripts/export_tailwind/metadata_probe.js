const staticFallbackMap = {
  "animate": { target_props: ["animation"], unit_kind: "Unitless" },
  "aspect": { target_props: ["aspect-ratio"], unit_kind: "Unitless" },
  "auto-cols": { target_props: ["grid-auto-columns"], unit_kind: "Unitless" },
  "auto-rows": { target_props: ["grid-auto-rows"], unit_kind: "Unitless" },
  "backdrop-blur": { target_props: ["backdrop-filter"], unit_kind: "Pixel", value_wrapper: "blur({})" },
  "backdrop-brightness": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "brightness({})" },
  "backdrop-contrast": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "contrast({})" },
  "backdrop-grayscale": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "grayscale({})" },
  "backdrop-hue-rotate": { target_props: ["backdrop-filter"], unit_kind: "Degree", value_wrapper: "hue-rotate({})" },
  "backdrop-invert": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "invert({})" },
  "backdrop-opacity": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "opacity({})" },
  "backdrop-saturate": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "saturate({})" },
  "backdrop-sepia": { target_props: ["backdrop-filter"], unit_kind: "Percentage", value_wrapper: "sepia({})" },
  "basis": { target_props: ["flex-basis"], unit_kind: "RemScale" },
  "blur": { target_props: ["filter"], unit_kind: "Pixel", value_wrapper: "blur({})" },
  "border": { target_props: ["border-width"], unit_kind: "Pixel" },
  "border-b": { target_props: ["border-bottom-width"], unit_kind: "Pixel" },
  "border-l": { target_props: ["border-left-width"], unit_kind: "Pixel" },
  "border-r": { target_props: ["border-right-width"], unit_kind: "Pixel" },
  "border-t": { target_props: ["border-top-width"], unit_kind: "Pixel" },
  "border-x": { target_props: ["border-left-width", "border-right-width"], unit_kind: "Pixel" },
  "border-y": { target_props: ["border-top-width", "border-bottom-width"], unit_kind: "Pixel" },
  "bottom": { target_props: ["bottom"], unit_kind: "RemScale" },
  "brightness": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "brightness({})" },
  "col": { target_props: ["grid-column"], unit_kind: "Unitless" },
  "col-end": { target_props: ["grid-column-end"], unit_kind: "Unitless" },
  "col-span": { target_props: ["grid-column"], unit_kind: "GridSpan" },
  "col-start": { target_props: ["grid-column-start"], unit_kind: "Unitless" },
  "columns": { target_props: ["columns"], unit_kind: "Unitless" },
  "container": { target_props: ["container-name"], unit_kind: "Unitless" },
  "container-name": { target_props: ["container-name"], unit_kind: "Unitless" },
  "content": { target_props: ["content"], unit_kind: "Unitless" },
  "contrast": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "contrast({})" },
  "cursor": { target_props: ["cursor"], unit_kind: "Unitless" },
  "delay": { target_props: ["transition-delay"], unit_kind: "Milliseconds" },
  "drop-shadow": { target_props: ["filter"], unit_kind: "Unitless", value_wrapper: "drop-shadow({})" },
  "duration": { target_props: ["transition-duration"], unit_kind: "Milliseconds" },
  "ease": { target_props: ["transition-timing-function"], unit_kind: "Unitless" },
  "fade-in": { target_props: ["--tw-enter-opacity"], unit_kind: "Percentage" },
  "fade-out": { target_props: ["--tw-exit-opacity"], unit_kind: "Percentage" },
  "gap": { target_props: ["gap"], unit_kind: "RemScale" },
  "gap-x": { target_props: ["column-gap"], unit_kind: "RemScale" },
  "gap-y": { target_props: ["row-gap"], unit_kind: "RemScale" },
  "grid-cols": { target_props: ["grid-template-columns"], unit_kind: "GridRepeat" },
  "grayscale": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "grayscale({})" },
  "grid-rows": { target_props: ["grid-template-rows"], unit_kind: "GridRepeat" },
  "grow": { target_props: ["flex-grow"], unit_kind: "Unitless" },
  "h": { target_props: ["height"], unit_kind: "RemScale" },
  "height": { target_props: ["height"], unit_kind: "RemScale" },
  "hue-rotate": { target_props: ["filter"], unit_kind: "Degree", value_wrapper: "hue-rotate({})" },
  "inset": { target_props: ["top", "right", "bottom", "left"], unit_kind: "RemScale" },
  "inset-x": { target_props: ["left", "right"], unit_kind: "RemScale" },
  "inset-y": { target_props: ["top", "bottom"], unit_kind: "RemScale" },
  "invert": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "invert({})" },
  "leading": { target_props: ["line-height"], unit_kind: "Unitless" },
  "left": { target_props: ["left"], unit_kind: "RemScale" },
  "line-clamp": { target_props: ["-webkit-line-clamp"], unit_kind: "Unitless" },
  "m": { target_props: ["margin"], unit_kind: "RemScale" },
  "margin": { target_props: ["margin"], unit_kind: "RemScale" },
  "max-h": { target_props: ["max-height"], unit_kind: "RemScale" },
  "max-w": { target_props: ["max-width"], unit_kind: "RemScale" },
  "mb": { target_props: ["margin-bottom"], unit_kind: "RemScale" },
  "min-h": { target_props: ["min-height"], unit_kind: "RemScale" },
  "min-w": { target_props: ["min-width"], unit_kind: "RemScale" },
  "ml": { target_props: ["margin-left"], unit_kind: "RemScale" },
  "mr": { target_props: ["margin-right"], unit_kind: "RemScale" },
  "mt": { target_props: ["margin-top"], unit_kind: "RemScale" },
  "mx": { target_props: ["margin-left", "margin-right"], unit_kind: "RemScale" },
  "my": { target_props: ["margin-top", "margin-bottom"], unit_kind: "RemScale" },
  "object": { target_props: ["object-fit"], unit_kind: "Unitless" },
  "opacity": { target_props: ["opacity"], unit_kind: "Percentage" },
  "order": { target_props: ["order"], unit_kind: "Unitless" },
  "origin": { target_props: ["transform-origin"], unit_kind: "Unitless" },
  "outline": { target_props: ["outline-width"], unit_kind: "Pixel" },
  "p": { target_props: ["padding"], unit_kind: "RemScale" },
  "padding": { target_props: ["padding"], unit_kind: "RemScale" },
  "pb": { target_props: ["padding-bottom"], unit_kind: "RemScale" },
  "pl": { target_props: ["padding-left"], unit_kind: "RemScale" },
  "pr": { target_props: ["padding-right"], unit_kind: "RemScale" },
  "pt": { target_props: ["padding-top"], unit_kind: "RemScale" },
  "px": { target_props: ["padding-left", "padding-right"], unit_kind: "RemScale" },
  "py": { target_props: ["padding-top", "padding-bottom"], unit_kind: "RemScale" },
  "right": { target_props: ["right"], unit_kind: "RemScale" },
  "rotate": { target_props: ["transform"], unit_kind: "Degree", value_wrapper: "rotate({})" },
  "rounded": { target_props: ["border-radius"], unit_kind: "RemScale" },
  "row": { target_props: ["grid-row"], unit_kind: "Unitless" },
  "row-end": { target_props: ["grid-row-end"], unit_kind: "Unitless" },
  "row-span": { target_props: ["grid-row"], unit_kind: "GridSpan" },
  "row-start": { target_props: ["grid-row-start"], unit_kind: "Unitless" },
  "saturate": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "saturate({})" },
  "scale": { target_props: ["transform"], unit_kind: "Percentage", value_wrapper: "scale({})" },
  "scale-x": { target_props: ["transform"], unit_kind: "Percentage", value_wrapper: "scaleX({})" },
  "scale-y": { target_props: ["transform"], unit_kind: "Percentage", value_wrapper: "scaleY({})" },
  "sepia": { target_props: ["filter"], unit_kind: "Percentage", value_wrapper: "sepia({})" },
  "shadow": { target_props: ["box-shadow"], unit_kind: "Unitless" },
  "shrink": { target_props: ["flex-shrink"], unit_kind: "Unitless" },
  "size": { target_props: ["width", "height"], unit_kind: "RemScale" },
  "skew-x": { target_props: ["transform"], unit_kind: "Degree", value_wrapper: "skewX({})" },
  "skew-y": { target_props: ["transform"], unit_kind: "Degree", value_wrapper: "skewY({})" },
  "slide-in-from-bottom": { target_props: ["--tw-enter-translate-y"], unit_kind: "RemScale" },
  "slide-in-from-left": { target_props: ["--tw-enter-translate-x"], unit_kind: "RemScale" },
  "slide-in-from-right": { target_props: ["--tw-enter-translate-x"], unit_kind: "RemScale" },
  "slide-in-from-top": { target_props: ["--tw-enter-translate-y"], unit_kind: "RemScale" },
  "slide-out-to-bottom": { target_props: ["--tw-exit-translate-y"], unit_kind: "RemScale" },
  "slide-out-to-left": { target_props: ["--tw-exit-translate-x"], unit_kind: "RemScale" },
  "slide-out-to-right": { target_props: ["--tw-exit-translate-x"], unit_kind: "RemScale" },
  "slide-out-to-top": { target_props: ["--tw-exit-translate-y"], unit_kind: "RemScale" },
  "spin-in": { target_props: ["--tw-enter-rotate"], unit_kind: "Degree" },
  "spin-out": { target_props: ["--tw-exit-rotate"], unit_kind: "Degree" },
  "top": { target_props: ["top"], unit_kind: "RemScale" },
  "tracking": { target_props: ["letter-spacing"], unit_kind: "RemScale" },
  "translate-x": { target_props: ["transform"], unit_kind: "RemScale", value_wrapper: "translateX({})" },
  "translate-y": { target_props: ["transform"], unit_kind: "RemScale", value_wrapper: "translateY({})" },
  "underline-offset": { target_props: ["text-underline-offset"], unit_kind: "Pixel" },
  "w": { target_props: ["width"], unit_kind: "RemScale" },
  "width": { target_props: ["width"], unit_kind: "RemScale" },
  "z": { target_props: ["z-index"], unit_kind: "Unitless" },
  "zoom-in": { target_props: ["--tw-enter-scale"], unit_kind: "Percentage" },
  "zoom-out": { target_props: ["--tw-exit-scale"], unit_kind: "Percentage" },
};

const { parseDeclarationList } = require('./css_utils');

/// 解析一段工具类 CSS 为 `Map<property, value>`（已剥离描述符 at-rule 与非法属性）
function parseDeclarations(cssStr) {
  const map = new Map();
  for (const [prop, val] of parseDeclarationList(cssStr)) {
    if (prop.startsWith('--tw-rotate') || prop.startsWith('--tw-scale') || prop.startsWith('--tw-skew') || prop.startsWith('--tw-translate')) {
      continue;
    }
    if (!map.has(prop)) map.set(prop, val);
  }
  return map;
}

function inferUnitKind(propVal) {
  if (propVal.includes('span ')) return 'GridSpan';
  if (propVal.includes('repeat(')) return 'GridRepeat';
  if (propVal.includes('deg')) return 'Degree';
  if (propVal.includes('ms') || propVal.endsWith('s')) return 'Milliseconds';
  if (propVal.includes('rem')) return 'RemScale';
  if (propVal.includes('px')) return 'Pixel';
  if (propVal.includes('%') || (propVal.startsWith('0.') && !isNaN(parseFloat(propVal)))) return 'Percentage';
  return null;
}

function inferPrefixMetadata(sortedPrefixKeys, designSystem) {
  const inferredMetadataObj = {};

  for (const prefixKey of sortedPrefixKeys) {
    const pKey = prefixKey.endsWith('-') ? prefixKey.slice(0, -1) : prefixKey;

    // 自动探针测试：用两个不同数值探测，只保留“值随输入变化”的声明。
    // 固定不变的声明（如 `border-inline-start-style: var(--tw-border-style)`）是伴生声明，
    // 不是数值目标属性，若混入 target_props 会导致 `border-s-[3px]` 产出 `*-style: 3px`。
    let inferred = null;
    const probeCandidates = [`${prefixKey}4`, `${prefixKey}2`, `${prefixKey}50`, `${prefixKey}150`, `${prefixKey}45`, `${prefixKey}1`];
    const probeCss = designSystem.candidatesToCss(probeCandidates);
    const parsed = probeCandidates.map((_, i) => {
      const cssStr = probeCss[i];
      if (!cssStr || typeof cssStr !== 'string' || cssStr.trim().length === 0) return null;
      const decls = parseDeclarations(cssStr);
      return decls.size > 0 ? decls : null;
    });

    const primaryIdx = parsed.findIndex(Boolean);
    if (primaryIdx !== -1) {
      const primary = parsed[primaryIdx];
      const other = parsed.find((d, i) => d && i !== primaryIdx && [...d].some(([p, v]) => primary.get(p) !== v));

      let props = [...primary.keys()];
      if (other) {
        const varying = props.filter(p => other.has(p) && other.get(p) !== primary.get(p));
        if (varying.length > 0) props = varying;
      }

      let unitKind = null;
      for (const p of props) {
        unitKind = inferUnitKind(primary.get(p));
        if (unitKind) break;
      }

      inferred = { target_props: Array.from(new Set(props)).sort(), unit_kind: unitKind || 'Unitless' };
    }

    // 优先采用 staticFallbackMap 权威配置，若无则使用自动探针推导结果
    if (staticFallbackMap[pKey]) {
      inferredMetadataObj[pKey] = staticFallbackMap[pKey];
    } else if (inferred) {
      inferredMetadataObj[pKey] = inferred;
    }
  }

  // 确保所有 staticFallbackMap 中的权威规则完全包含
  for (const [sKey, sMeta] of Object.entries(staticFallbackMap)) {
    if (!inferredMetadataObj[sKey]) {
      inferredMetadataObj[sKey] = sMeta;
    }
  }

  return inferredMetadataObj;
}

module.exports = {
  staticFallbackMap,
  inferPrefixMetadata,
};
