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
        familyShades.push({
          shade: s,
          raw: entry.value,
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
};
