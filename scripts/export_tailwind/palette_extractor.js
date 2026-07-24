function oklchToHexAndRgb(oklchStr) {
  const match = oklchStr.match(/oklch\(\s*([\d.%]+)\s+([\d.|none]+)\s+([\d.|none]+)(?:\s*\/\s*([\d.%]+))?\s*\)/);
  if (!match) {
    return {
      hex: oklchStr,
      raw: oklchStr,
      rgb: [0, 0, 0],
    };
  }

  let l = parseFloat(match[1]) / (match[1].endsWith('%') ? 100 : 1);
  let c = match[2] === 'none' ? 0 : parseFloat(match[2]);
  let h = match[3] === 'none' ? 0 : parseFloat(match[3]);

  const hRad = (h * Math.PI) / 180;
  const a = c * Math.cos(hRad);
  const b = c * Math.sin(hRad);

  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.2914855480 * b;

  const l3 = l_ * l_ * l_;
  const m3 = m_ * m_ * m_;
  const s3 = s_ * s_ * s_;

  let rLin = +4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
  let gLin = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
  let bLin = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

  const toSrgb = (v) => {
    v = Math.max(0, Math.min(1, v));
    return v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
  };

  const r = Math.round(toSrgb(rLin) * 255);
  const g = Math.round(toSrgb(gLin) * 255);
  const b_ = Math.round(toSrgb(bLin) * 255);

  const toHex = (n) => n.toString(16).padStart(2, '0');
  const hex = '#' + toHex(r) + toHex(g) + toHex(b_);

  return {
    hex,
    raw: oklchStr,
    rgb: [r, g, b_],
  };
}

function extractPalette(designSystem) {
  const palette = {};
  const shades = ['50', '100', '200', '300', '400', '500', '600', '700', '800', '900', '950'];
  const families = new Set();

  for (const k of designSystem.theme.values.keys()) {
    if (k.startsWith('--color-')) {
      const parts = k.replace('--color-', '').split('-');
      if (parts.length === 2 && shades.includes(parts[1])) {
        families.add(parts[0]);
      }
    }
  }

  const sortedFamilies = Array.from(families).sort();
  for (const f of sortedFamilies) {
    const familyShades = [];
    for (const s of shades) {
      const entry = designSystem.theme.values.get(`--color-${f}-${s}`);
      if (entry && entry.value) {
        const rawVal = entry.value;
        const colorInfo = oklchToHexAndRgb(rawVal);
        familyShades.push({
          shade: s,
          hex: colorInfo.hex,
          raw: colorInfo.raw,
          rgb: colorInfo.rgb,
        });
      }
    }
    if (familyShades.length === 11) {
      palette[f] = familyShades;
    }
  }

  return palette;
}

module.exports = {
  extractPalette,
  oklchToHexAndRgb,
};
