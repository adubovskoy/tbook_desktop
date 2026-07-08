// Port of `ui/reader/BookProgress.kt`.

function clamp01(v: number): number {
  return Math.max(0, Math.min(1, v));
}

/**
 * Fraction of the whole book read, in [0,1], weighting chapters by `sizes`.
 * `chapterFraction` is progress within the current chapter, in [0,1].
 */
export function bookProgress(
  sizes: number[],
  chapterIndex: number,
  chapterFraction: number,
): number {
  const total = sizes.reduce((a, b) => a + b, 0);
  if (total <= 0 || chapterIndex < 0 || chapterIndex >= sizes.length) return 0;
  const before = sizes.slice(0, chapterIndex).reduce((a, b) => a + b, 0);
  const within = sizes[chapterIndex] * clamp01(chapterFraction);
  return clamp01((before + within) / total);
}
