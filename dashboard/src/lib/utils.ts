export function cn(...classes: (string | undefined | null | false)[]) {
  return classes
    .filter((c) => typeof c === 'string')
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim()
}
