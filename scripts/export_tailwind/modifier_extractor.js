function remToPx(remStr) {
  const num = parseFloat(remStr);
  return Math.round(num * 16);
}

function extractModifiers(ds) {
  const allVariants = ds.getVariants();
  const modifiers = [];

  for (const variant of allVariants) {
    const key = variant.name;

    if (key === '*') {
      modifiers.push({ key: '*', kind: 'Child', priority: 10, css_selector: '& > *' });
      continue;
    }
    if (key === '**') {
      modifiers.push({ key: '**', kind: 'Descendant', priority: 10, css_selector: '& *' });
      continue;
    }
    if (key === 'dark') {
      modifiers.push({ key: 'dark', kind: 'Dark', priority: 60, css_selector: '.dark &, &.dark' });
      continue;
    }

    const cssArr = ds.candidatesToCss([`${key}:block`]);
    const css = Array.isArray(cssArr) ? cssArr.join('\n') : (cssArr || '');

    if (!css) continue;

    // 1. Media Breakpoints (@media (width >= 40rem) or @media (min-width: 640px))
    const mediaWidthMatch = css.match(/@media\s*\(\s*width\s*>=\s*([\d\.]+)rem\s*\)/i);
    if (mediaWidthMatch) {
      const px = remToPx(mediaWidthMatch[1]);
      modifiers.push({
        key,
        kind: 'MediaBreakpoint',
        priority: 1000 + px,
        css_selector: `(min-width: ${px}px)`
      });
      continue;
    }

    const minWidthMatch = css.match(/@media\s*\(\s*min-width\s*:\s*([\d\.]+)(px|rem)\s*\)/i);
    if (minWidthMatch) {
      const val = parseFloat(minWidthMatch[1]);
      const px = minWidthMatch[2] === 'rem' ? Math.round(val * 16) : Math.round(val);
      modifiers.push({
        key,
        kind: 'MediaBreakpoint',
        priority: 1000 + px,
        css_selector: `(min-width: ${px}px)`
      });
      continue;
    }

    // 2. 选择器型变体：伪元素、伪类，以及 rtl/ltr/open/inert 之类的复合 `:where(...)` 选择器。
    //    优先于媒体特性判定——Tailwind 会把 `hover:` 包进 `@media (hover: hover)`，
    //    但它的本质仍是伪类变体。
    const escapedKey = key.replace(/[*+?^${}()|[\]\\]/g, '\\$&');
    const ruleMatch = css.match(new RegExp(`\\.${escapedKey}\\\\:block([^{]*)\\{`));
    const selectorSuffix = ruleMatch ? ruleMatch[1].trim() : '';

    if (selectorSuffix) {
      // 简单伪元素 `::before`
      if (/^::[a-zA-Z-]+$/.test(selectorSuffix)) {
        modifiers.push({ key, kind: 'PseudoElement', priority: 20, css_selector: `&${selectorSuffix}` });
        continue;
      }
      // 简单伪类 `:hover` 与仅含简单参数的函数式伪类 `:nth-child(even)`
      if (/^:[a-zA-Z-]+(\([a-zA-Z0-9+\-\s]*\))?$/.test(selectorSuffix)) {
        modifiers.push({ key, kind: 'PseudoClass', priority: 20, css_selector: `&${selectorSuffix}` });
        continue;
      }
      // 复合选择器 `:where(:dir(rtl), [dir="rtl"], ...)` / `:is([open], :popover-open, :open)`
      modifiers.push({ key, kind: 'SelectorVariant', priority: 25, css_selector: `&${selectorSuffix}` });
      continue;
    }

    // 3. 纯媒体特性变体 (print / motion-reduce / motion-safe / forced-colors /
    //    contrast-more / portrait / landscape / pointer-* ...)。这些变体此前被整段丢弃，
    //    导致宏侧兜底成 `:print` 之类永不匹配的伪类。
    const mediaMatch = css.match(/@media\s+([^{]+?)\s*\{/);
    if (mediaMatch) {
      modifiers.push({
        key,
        kind: 'MediaFeature',
        priority: 65,
        css_selector: mediaMatch[1].trim()
      });
    }
  }

  // 按 key 字典序升序排序，确保 Rust 二分查找 (binary_search_by_key) 正常有效
  modifiers.sort((a, b) => a.key.localeCompare(b.key));
  return modifiers;
}

module.exports = {
  extractModifiers,
};
