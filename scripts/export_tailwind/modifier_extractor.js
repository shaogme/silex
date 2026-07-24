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

    // 2. Pseudo Elements & Pseudo Classes
    const escapedKey = key.replace(/[*+?^${}()|[\]\\]/g, '\\$&');
    const ruleMatch = css.match(new RegExp(`(?:\\.${escapedKey}\\\\:block)(\\S+)\\s*\\{`));
    if (ruleMatch) {
      const selectorSuffix = ruleMatch[1];
      if (selectorSuffix.startsWith('::')) {
        modifiers.push({
          key,
          kind: 'PseudoElement',
          priority: 20,
          css_selector: `&${selectorSuffix}`
        });
        continue;
      } else if (selectorSuffix.startsWith(':')) {
        modifiers.push({
          key,
          kind: 'PseudoClass',
          priority: 20,
          css_selector: `&${selectorSuffix}`
        });
        continue;
      }
    }
  }

  // 按 key 字典序升序排序，确保 Rust 二分查找 (binary_search_by_key) 正常有效
  modifiers.sort((a, b) => a.key.localeCompare(b.key));
  return modifiers;
}

module.exports = {
  extractModifiers,
};
