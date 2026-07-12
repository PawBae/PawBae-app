// 联调基建的最小 Node 环境声明。桌面 tsconfig 面向 DOM，不为几份只在
// vitest node 环境运行的联调文件引入 @types/node——这里就地声明用到的最小面。
declare module 'node:child_process' {
  export function execFileSync(
    file: string,
    args: readonly string[],
    options: { encoding: 'utf8' },
  ): string;
}
