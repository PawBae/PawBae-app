#!/usr/bin/env node
// 版本号单源 bump：四处版本号（tauri.conf.json / package.json / Cargo.toml /
// Cargo.lock）一次改齐，杜绝手动同步漂移。零依赖，setup-node 即可跑。
//
//   node scripts/bump-version.mjs 0.3.0          # bump 到 0.3.0
//   node scripts/bump-version.mjs --check        # 校验四处一致（漂移即非零退出）
//   node scripts/bump-version.mjs --check v0.3.0 # 额外校验等于期望值（tag 门禁用）
//
// 改法是「按当前值做精确文本替换」而不是 JSON.parse+stringify——不重排文件、
// diff 只有版本行；替换后重新解析断言生效，替换数不等于 1 直接失败。

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const SEMVER = /^\d+\.\d+\.\d+$/;

const TAURI_CONF = 'apps/desktop/src-tauri/tauri.conf.json';
const PACKAGE_JSON = 'apps/desktop/package.json';
const CARGO_TOML = 'apps/desktop/src-tauri/Cargo.toml';
const CARGO_LOCK = 'apps/desktop/src-tauri/Cargo.lock';

const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');
const fail = (msg) => {
  console.error(`bump-version: ${msg}`);
  process.exit(1);
};

function crateName() {
  const m = read(CARGO_TOML).match(/^name = "([^"]+)"/m);
  if (!m) fail(`${CARGO_TOML} 里找不到 [package] name`);
  return m[1];
}

/** 四处版本号的当前值（Cargo.lock 取本 crate 的条目）。 */
function currentVersions() {
  const crate = crateName();
  const lockEntry = read(CARGO_LOCK).match(
    new RegExp(`name = "${crate}"\\nversion = "([^"]+)"`),
  );
  return {
    [TAURI_CONF]: JSON.parse(read(TAURI_CONF)).version,
    [PACKAGE_JSON]: JSON.parse(read(PACKAGE_JSON)).version,
    [CARGO_TOML]: read(CARGO_TOML).match(/^version = "([^"]+)"/m)?.[1],
    [CARGO_LOCK]: lockEntry?.[1],
  };
}

function check(expected) {
  const versions = currentVersions();
  const values = [...new Set(Object.values(versions))];
  if (values.length !== 1 || values[0] === undefined) {
    for (const [file, v] of Object.entries(versions)) console.error(`  ${file}: ${v}`);
    fail('版本号已漂移——用本脚本 bump，不要手改单个文件');
  }
  if (expected !== undefined && values[0] !== expected) {
    fail(`版本号是 ${values[0]}，期望 ${expected}（tag 与 tauri.conf.json 必须一致）`);
  }
  console.log(`version ${values[0]} — 四处一致${expected ? '且与期望相符' : ''}`);
}

/** 把 rel 文件里恰好 count 次出现的 needle 换成 replacement，次数不符即失败。 */
function replaceExact(rel, needle, replacement, count) {
  const text = read(rel);
  const found = text.split(needle).length - 1;
  if (found !== count) {
    fail(`${rel} 里 ${JSON.stringify(needle)} 出现 ${found} 次（期望 ${count}）——文件结构变了，先更新本脚本`);
  }
  writeFileSync(join(ROOT, rel), text.replaceAll(needle, replacement));
}

function bump(next) {
  const versions = currentVersions();
  const values = [...new Set(Object.values(versions))];
  if (values.length !== 1) {
    for (const [file, v] of Object.entries(versions)) console.error(`  ${file}: ${v}`);
    fail('版本号已漂移，先人工对齐再 bump');
  }
  const current = values[0];
  if (current === next) fail(`已经是 ${next}`);

  const crate = crateName();
  // JSON 两处：根级 "version" 行。依赖项版本号撞车的概率极低，但 count=1 硬断言兜底
  replaceExact(TAURI_CONF, `"version": "${current}"`, `"version": "${next}"`, 1);
  replaceExact(PACKAGE_JSON, `"version": "${current}"`, `"version": "${next}"`, 1);
  // Cargo.toml：[package] 区首个 version 行
  replaceExact(CARGO_TOML, `version = "${current}"`, `version = "${next}"`, 1);
  // Cargo.lock：本 crate 的 name+version 相邻两行（避免撞上同版本号的依赖）
  replaceExact(
    CARGO_LOCK,
    `name = "${crate}"\nversion = "${current}"`,
    `name = "${crate}"\nversion = "${next}"`,
    1,
  );

  console.log(`${current} → ${next}`);
  console.log('别忘了 CHANGELOG.md，然后走 PR 合并（不要直推 main）——见 docs/RELEASING.md');
}

const [arg, extra] = process.argv.slice(2);
if (arg === '--check') {
  const expected = extra === undefined ? undefined : extra.replace(/^v/, '');
  if (expected !== undefined && !SEMVER.test(expected)) fail(`期望值不是 x.y.z：${extra}`);
  check(expected);
} else if (arg !== undefined && SEMVER.test(arg)) {
  bump(arg);
} else {
  fail('用法: bump-version.mjs <x.y.z> | --check [vX.Y.Z]');
}
