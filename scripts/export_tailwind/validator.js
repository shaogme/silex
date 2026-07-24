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

module.exports = {
  filterValidClasses,
};
