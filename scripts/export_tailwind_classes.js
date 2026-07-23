const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

async function filterValidClasses(candidateSet, designSystem, chunkSize = 3000) {
  console.log(`开始二次过滤，待校验类名数量: ${candidateSet.size}...`);

  const alwaysKeep = new Set([
    'animate-in',
    'animate-out',
    'break-inside-avoid-flex',
    'break-after-avoid-flex',
    'break-before-avoid-flex',
  ]);

  const rawCandidates = Array.from(candidateSet);
  const validClasses = new Set();

  for (let i = 0; i < rawCandidates.length; i += chunkSize) {
    const chunk = rawCandidates.slice(i, i + chunkSize);
    const results = designSystem.candidatesToCss(chunk);

    for (let j = 0; j < chunk.length; j++) {
      const candidate = chunk[j];
      const generatedCss = results[j];

      if (alwaysKeep.has(candidate) || (generatedCss && typeof generatedCss === 'string' && generatedCss.trim().length > 0)) {
        validClasses.add(candidate);
      }
    }
  }

  console.log(`过滤完成！有效类名: ${validClasses.size} 个，丢弃无效类名: ${candidateSet.size - validClasses.size} 个`);
  return validClasses;
}

async function main() {
  console.log('检查并准备依赖...');

  let tailwind;
  try {
    tailwind = require('tailwindcss');
  } catch (e) {
    console.log('未检测到 tailwindcss，正在自动安装最新版...');
    execSync('npm install --no-save tailwindcss@latest', { stdio: 'inherit' });
    tailwind = require('tailwindcss');
  }

  console.log('正在加载 Tailwind CSS v4 设计系统...');

  // 1. 加载设计系统
  let designSystem;
  if (typeof tailwind.__unstable__loadDesignSystem === 'function') {
    const tailwindDir = path.dirname(require.resolve('tailwindcss/package.json'));

    designSystem = await tailwind.__unstable__loadDesignSystem('@import "tailwindcss";', {
      loadStylesheet: async (id, base) => {
        let targetPath;
        if (id === 'tailwindcss' || id === 'tailwindcss/index.css') {
          targetPath = path.join(tailwindDir, 'index.css');
        } else if (id.startsWith('tailwindcss/')) {
          targetPath = path.join(tailwindDir, id.replace('tailwindcss/', ''));
        } else if (base) {
          targetPath = path.resolve(base, id);
        }

        if (targetPath && fs.existsSync(targetPath)) {
          const content = fs.readFileSync(targetPath, 'utf-8');
          return { content, base: path.dirname(targetPath) };
        }
        return { content: '', base };
      }
    });
  } else {
    throw new Error('未在 tailwindcss 中找到 __unstable__loadDesignSystem 方法，请确认安装的版本为 v4.x');
  }

  console.log('正在提取类名与动态前缀...');
  const rawClassList = designSystem.getClassList();
  
  const classSet = new Set();
  const candidateDynamicPairs = [];

  function registerDynamicSuffix(prefix, suffix) {
    if (!prefix || !suffix) return;
    const prefixKey = prefix.endsWith('-') ? prefix : `${prefix}-`;
    const cls = `${prefixKey}${suffix}`;
    candidateDynamicPairs.push({ cls, prefixKey, suffix: String(suffix) });
  }

  // 2. 解析原生解析出的 class 规则
  for (const entry of rawClassList) {
    if (typeof entry === 'string') {
      classSet.add(entry);
    } else if (Array.isArray(entry)) {
      const [prefix, options] = entry;
      if (prefix) {
        classSet.add(prefix);
      }
      if (options && typeof options === 'object' && options.values) {
        let valList = [];
        if (Array.isArray(options.values)) {
          valList = options.values;
        } else if (typeof options.values === 'object') {
          valList = Object.keys(options.values);
        }
        for (const val of valList) {
          if (val === 'DEFAULT' || val === null || val === undefined) {
            if (prefix) classSet.add(prefix);
          } else if (prefix) {
            const cls = `${prefix}-${val}`;
            classSet.add(cls);
            registerDynamicSuffix(prefix, String(val));
          }
        }
      }
    }
  }

  // 3. 补充标准 Tailwind CSS 尺寸与通用后缀
  const sizeSuffixes = ['3xs', '2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl', '6xl', '7xl', '8xl', '9xl', 'full', 'auto', 'none', 'px', 'screen', 'fit', 'max', 'min', '1/2', '1/3', '2/3', '1/4', '3/4'];
  const textSizeSuffixes = ['3xs', '2xs', 'xs', 'sm', 'base', 'md', 'lg', 'xl', '2xl', '3xl', '4xl', '5xl', '6xl', '7xl', '8xl', '9xl', 'full', 'auto', 'none', 'px'];
  const numSuffixes = ['0', '0.5', '1', '1.5', '2', '2.5', '3', '3.5', '4', '5', '6', '7', '8', '9', '10', '11', '12', '14', '16', '20', '24', '28', '32', '36', '40', '44', '48', '52', '56', '60', '64', '72', '80', '96'];
  const fontWeightSuffixes = ['thin', 'extralight', 'light', 'normal', 'medium', 'semibold', 'bold', 'extrabold', 'black', 'sans', 'serif', 'mono'];
  const fontStretchSuffixes = ['normal', 'condensed', 'expanded', 'ultra-condensed', 'extra-condensed', 'semi-condensed', 'semi-expanded', 'extra-expanded', 'ultra-expanded'];
  const leadingSuffixes = ['none', 'tight', 'snug', 'normal', 'relaxed', 'loose', 'px'];
  const trackingSuffixes = ['tighter', 'tight', 'normal', 'wide', 'wider', 'widest'];
  const animateSuffixes = ['spin', 'ping', 'pulse', 'bounce', 'none', 'in', 'out'];
  const blurSuffixes = ['none', '3xs', '2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl'];
  const shadowSuffixes = ['2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', 'inner', 'none', 'initial'];
  const roundedSuffixes = ['none', '3xs', '2xs', 'xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl', 'full'];
  const borderSuffixes = ['0', '1', '2', '4', '8', 'none', 'solid', 'dashed', 'dotted', 'double', 'hidden'];
  const breakSuffixes = ['auto', 'avoid', 'avoid-page', 'avoid-column', 'avoid-flex'];
  const gradientSuffixes = ['gradient-to-r', 'gradient-to-l', 'gradient-to-t', 'gradient-to-b', 'gradient-to-tr', 'gradient-to-br', 'gradient-to-tl', 'gradient-to-bl', 'linear-to-r', 'linear-to-l', 'linear-to-t', 'linear-to-b', 'linear-to-tr', 'linear-to-br', 'linear-to-tl', 'linear-to-bl', 'radial', 'conic', 'none'];
  const opacitySuffixes = ['0', '5', '10', '20', '25', '30', '40', '50', '60', '70', '75', '80', '90', '95', '100'];
  const durationSuffixes = ['75', '100', '150', '200', '300', '500', '700', '1000'];
  const scaleSuffixes = ['0', '50', '75', '90', '95', '100', '105', '110', '125', '150'];
  const rotateSuffixes = ['0', '45', '90', '180', '-45', '-90', '-180'];
  const translateSuffixes = ['0', 'full', '-full', '1/2', '-1/2'];
  const zIndexSuffixes = ['0', '10', '20', '30', '40', '50', 'auto'];

  const prefixMaps = [
    { prefixes: ['font'], suffixes: fontWeightSuffixes.concat(fontStretchSuffixes) },
    { prefixes: ['leading'], suffixes: leadingSuffixes.concat(numSuffixes) },
    { prefixes: ['tracking'], suffixes: trackingSuffixes },
    { prefixes: ['text'], suffixes: textSizeSuffixes.concat(numSuffixes) },
    { prefixes: ['animate'], suffixes: animateSuffixes },
    { prefixes: ['blur', 'backdrop-blur'], suffixes: blurSuffixes },
    { prefixes: ['shadow', 'inset-shadow'], suffixes: shadowSuffixes },
    { prefixes: ['rounded', 'rounded-t', 'rounded-r', 'rounded-b', 'rounded-l', 'rounded-tl', 'rounded-tr', 'rounded-br', 'rounded-bl', 'rounded-s', 'rounded-e', 'rounded-ss', 'rounded-se', 'rounded-es', 'rounded-ee'], suffixes: roundedSuffixes },
    { prefixes: ['border', 'border-t', 'border-r', 'border-b', 'border-l', 'border-x', 'border-y', 'border-s', 'border-e', 'border-bs', 'border-be'], suffixes: borderSuffixes },
    { prefixes: ['break-inside', 'break-before', 'break-after'], suffixes: breakSuffixes },
    { prefixes: ['bg'], suffixes: gradientSuffixes },
    { prefixes: ['max-w', 'min-w', 'max-h', 'min-h', 'w', 'h', 'columns', 'gap', 'p', 'm', 'px', 'py', 'pt', 'pr', 'pb', 'pl', 'mx', 'my', 'mt', 'mr', 'mb', 'ml', 'inset', 'top', 'right', 'bottom', 'left'], suffixes: sizeSuffixes.concat(numSuffixes) },
    { prefixes: ['opacity'], suffixes: opacitySuffixes },
    { prefixes: ['duration', 'delay'], suffixes: durationSuffixes },
    { prefixes: ['scale'], suffixes: scaleSuffixes },
    { prefixes: ['rotate'], suffixes: rotateSuffixes },
    { prefixes: ['translate-x', 'translate-y'], suffixes: translateSuffixes },
    { prefixes: ['z'], suffixes: zIndexSuffixes },
  ];

  for (const { prefixes, suffixes } of prefixMaps) {
    for (const p of prefixes) {
      for (const s of suffixes) {
        if (s) {
          const cls = `${p}-${s}`;
          classSet.add(cls);
          registerDynamicSuffix(p, s);
        }
      }
    }
  }

  // 3.5. 二次过滤，剔除合成了但 Tailwind 无法编译的非法类名（按每批 3000 个分批）
  const filteredClassSet = await filterValidClasses(classSet, designSystem, 3000);

  // 4. 清理并排序
  const classList = Array.from(filteredClassSet)
    .filter(cls => typeof cls === 'string' && cls.trim().length > 0)
    .filter(cls => !cls.startsWith('space-') && !cls.startsWith('-space-') && !cls.startsWith('divide-') && !cls.startsWith('-divide-'))
    .sort();

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
  for (const key of sortedPrefixKeys) {
    const suffixes = Array.from(validDynamicPrefixesMap.get(key)).sort();
    if (suffixes.length > 0) {
      dynamicPrefixesObj[key] = suffixes;
    }
  }

  // 5. 确保导出目录存在
  const outputFile = path.join(__dirname, '../crates/utils/silex_codegen/tailwind-classes.json');
  const outputDir = path.dirname(outputFile);
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  const exportData = {
    classes: classList,
    dynamic_prefixes: dynamicPrefixesObj,
  };

  fs.writeFileSync(outputFile, JSON.stringify(exportData, null, 2), 'utf-8');

  console.log(`导出成功！文件已保存至: ${outputFile}`);
  console.log(`共包含类名数量：${classList.length}`);
  console.log(`包含动态前缀数量：${Object.keys(dynamicPrefixesObj).length}`);
}

main().catch(err => {
  console.error('执行失败：', err);
  process.exit(1);
});