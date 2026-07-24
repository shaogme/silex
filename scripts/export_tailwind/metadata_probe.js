const staticFallbackMap = {
  "auto-cols": { target_props: ["grid-auto-columns"], unit_kind: "Unitless" },
  "auto-rows": { target_props: ["grid-auto-rows"], unit_kind: "Unitless" },
  "backdrop-blur": { target_props: ["backdrop-filter"], unit_kind: "Pixel" },
  "blur": { target_props: ["filter"], unit_kind: "Pixel" },
  "border": { target_props: ["border-width"], unit_kind: "Pixel" },
  "border-b": { target_props: ["border-bottom-width"], unit_kind: "Pixel" },
  "border-l": { target_props: ["border-left-width"], unit_kind: "Pixel" },
  "border-r": { target_props: ["border-right-width"], unit_kind: "Pixel" },
  "border-t": { target_props: ["border-top-width"], unit_kind: "Pixel" },
  "border-x": { target_props: ["border-left-width", "border-right-width"], unit_kind: "Pixel" },
  "border-y": { target_props: ["border-top-width", "border-bottom-width"], unit_kind: "Pixel" },
  "bottom": { target_props: ["bottom"], unit_kind: "RemScale" },
  "col-end": { target_props: ["grid-column-end"], unit_kind: "Unitless" },
  "col-span": { target_props: ["grid-column"], unit_kind: "GridSpan" },
  "col-start": { target_props: ["grid-column-start"], unit_kind: "Unitless" },
  "columns": { target_props: ["column-count"], unit_kind: "Unitless" },
  "delay": { target_props: ["transition-delay"], unit_kind: "Milliseconds" },
  "duration": { target_props: ["transition-duration"], unit_kind: "Milliseconds" },
  "fade-in": { target_props: ["--tw-enter-opacity"], unit_kind: "Percentage" },
  "fade-out": { target_props: ["--tw-exit-opacity"], unit_kind: "Percentage" },
  "gap": { target_props: ["gap"], unit_kind: "RemScale" },
  "gap-x": { target_props: ["column-gap"], unit_kind: "RemScale" },
  "gap-y": { target_props: ["row-gap"], unit_kind: "RemScale" },
  "grid-cols": { target_props: ["grid-template-columns"], unit_kind: "GridRepeat" },
  "grid-rows": { target_props: ["grid-template-rows"], unit_kind: "GridRepeat" },
  "h": { target_props: ["height"], unit_kind: "RemScale" },
  "height": { target_props: ["height"], unit_kind: "RemScale" },
  "inset": { target_props: ["top", "right", "bottom", "left"], unit_kind: "RemScale" },
  "inset-x": { target_props: ["left", "right"], unit_kind: "RemScale" },
  "inset-y": { target_props: ["top", "bottom"], unit_kind: "RemScale" },
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
  "opacity": { target_props: ["opacity"], unit_kind: "Percentage" },
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
  "rotate": { target_props: ["transform"], unit_kind: "Degree" },
  "row-end": { target_props: ["grid-row-end"], unit_kind: "Unitless" },
  "row-span": { target_props: ["grid-row"], unit_kind: "GridSpan" },
  "row-start": { target_props: ["grid-row-start"], unit_kind: "Unitless" },
  "scale": { target_props: ["transform"], unit_kind: "Percentage" },
  "scale-x": { target_props: ["transform"], unit_kind: "Percentage" },
  "scale-y": { target_props: ["transform"], unit_kind: "Percentage" },
  "size": { target_props: ["width", "height"], unit_kind: "RemScale" },
  "skew-x": { target_props: ["transform"], unit_kind: "Degree" },
  "skew-y": { target_props: ["transform"], unit_kind: "Degree" },
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
  "translate-x": { target_props: ["transform"], unit_kind: "RemScale" },
  "translate-y": { target_props: ["transform"], unit_kind: "RemScale" },
  "underline-offset": { target_props: ["text-underline-offset"], unit_kind: "Pixel" },
  "w": { target_props: ["width"], unit_kind: "RemScale" },
  "width": { target_props: ["width"], unit_kind: "RemScale" },
  "z": { target_props: ["z-index"], unit_kind: "Unitless" },
  "zoom-in": { target_props: ["--tw-enter-scale"], unit_kind: "Percentage" },
  "zoom-out": { target_props: ["--tw-exit-scale"], unit_kind: "Percentage" },
};

function inferPrefixMetadata(sortedPrefixKeys, designSystem) {
  const inferredMetadataObj = {};

  for (const prefixKey of sortedPrefixKeys) {
    const pKey = prefixKey.endsWith('-') ? prefixKey.slice(0, -1) : prefixKey;
    
    // 自动探针测试
    let inferred = null;
    const probeCandidates = [`${prefixKey}4`, `${prefixKey}2`, `${prefixKey}50`, `${prefixKey}150`, `${prefixKey}45`, `${prefixKey}1`].filter(Boolean);
    
    if (probeCandidates.length > 0) {
      const probeCss = designSystem.candidatesToCss(probeCandidates);
      for (let i = 0; i < probeCandidates.length; i++) {
        const cssStr = probeCss[i];
        if (cssStr && typeof cssStr === 'string' && cssStr.trim().length > 0) {
          // 解析 CSS 声明
          const declMatches = Array.from(cssStr.matchAll(/([a-zA-Z0-9_\-]+)\s*:\s*([^;]+);/g));
          const props = [];
          let unitKind = null;

          for (const match of declMatches) {
            const propName = match[1];
            const propVal = match[2].trim();

            if (propName.startsWith('--tw-rotate') || propName.startsWith('--tw-scale') || propName.startsWith('--tw-skew') || propName.startsWith('--tw-translate')) {
              continue;
            }
            props.push(propName);

            if (!unitKind) {
              if (propVal.includes('span ')) unitKind = 'GridSpan';
              else if (propVal.includes('repeat(')) unitKind = 'GridRepeat';
              else if (propVal.includes('deg')) unitKind = 'Degree';
              else if (propVal.includes('ms') || propVal.endsWith('s')) unitKind = 'Milliseconds';
              else if (propVal.includes('rem')) unitKind = 'RemScale';
              else if (propVal.includes('px')) unitKind = 'Pixel';
              else if (propVal.includes('%') || (propVal.startsWith('0.') && !isNaN(parseFloat(propVal)))) unitKind = 'Percentage';
            }
          }

          if (props.length > 0) {
            inferred = { target_props: Array.from(new Set(props)).sort(), unit_kind: unitKind || 'Unitless' };
            break;
          }
        }
      }
    }

    // 优先采用自动探针，若为通用属性则 fallback 至完整定义的 SSOT
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
