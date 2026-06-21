// Insert a null row wherever the timeline has a jump (missing data), so charts
// break the line there instead of drawing a straight bridge across the gap.
// uPlot only breaks at explicit nulls; a plain time jump between two adjacent
// array points would otherwise be connected regardless of how far apart they are.
//
// t: unix-seconds timeline; arrays: parallel data arrays aligned to t.
// Returns { t, arrays } with a single null sample inserted at each gap.
export function insertGaps(t, arrays) {
  if (!t || t.length < 3) return { t, arrays }
  let step = Infinity
  for (let i = 1; i < t.length; i++) {
    const d = t[i] - t[i - 1]
    if (d > 0 && d < step) step = d
  }
  if (!isFinite(step)) return { t, arrays }
  const thr = step * 2.5 // only break on a clear multi-step gap, tolerate jitter
  const nt = []
  const na = arrays.map(() => [])
  for (let i = 0; i < t.length; i++) {
    if (i > 0 && t[i] - t[i - 1] > thr) {
      nt.push(t[i - 1] + step)
      na.forEach((a) => a.push(null))
    }
    nt.push(t[i])
    arrays.forEach((a, k) => na[k].push(a[i]))
  }
  return { t: nt, arrays: na }
}
