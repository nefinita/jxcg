/** 简易 className 合并（对齐 jpcg）。 */
export default function clsx(...args: (string | false | null | undefined)[]): string {
  return args.filter(Boolean).join(" ");
}
