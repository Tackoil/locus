// Adapt presentation only; keep the repo's geometric constructions and assertions.
const colors = {
  '#111C2B': '#FFFFFF',
  '#E7EFF7': '#202020',
  '#9AAFC3': '#747474',
  '#B5C6D6': '#334155',
  '#60758D': '#94A3B8',
  '#5EEAD4': '#4F46E5',
  '#F6CC73': '#4B5563',
  '#A9A6FF': '#64748B',
};

export function themeHeroSource(source) {
  return source
    .replace(/^\/\/ A native Locus rectangle[^\n]*\n/m, '')
    .replace(/^rect backdrop\([^\n]*\n/m, '')
    .replace(/^rel center\(circum, backdrop\)\n/m, '')
    .replace(/^label series\([\s\S]*?(?=^\/\/ Independent theorem assertions;)/m, '')
    .replace(/#[\da-f]{6}/gi, color => colors[color.toUpperCase()] ?? color);
}
