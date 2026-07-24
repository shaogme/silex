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

module.exports = {
  sizeSuffixes,
  textSizeSuffixes,
  numSuffixes,
  fontWeightSuffixes,
  fontStretchSuffixes,
  leadingSuffixes,
  trackingSuffixes,
  animateSuffixes,
  blurSuffixes,
  shadowSuffixes,
  roundedSuffixes,
  borderSuffixes,
  breakSuffixes,
  gradientSuffixes,
  opacitySuffixes,
  durationSuffixes,
  scaleSuffixes,
  rotateSuffixes,
  translateSuffixes,
  zIndexSuffixes,
  prefixMaps,
};
