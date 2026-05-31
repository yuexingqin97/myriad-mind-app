// ---- cn() — className 条件拼接 ----
// cc-switch 的简化版。没有 Tailwind → 不需要 twMerge，只做条件拼接。

export function cn(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}
