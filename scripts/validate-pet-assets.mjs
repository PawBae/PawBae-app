import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const root = new URL('../apps/desktop/public/assets/builtin/', import.meta.url);
const manifest = JSON.parse(await readFile(new URL('pets-manifest.json', root), 'utf8'));
const requiredSoluStates = [
  'idle',
  'running',
  'working',
  'waiting',
  'happy',
  'sleep',
  'arrival',
  'return',
];

function pngSize(bytes) {
  const signature = bytes.subarray(0, 8).toString('hex');
  if (signature !== '89504e470d0a1a0a') throw new Error('expected a PNG spritesheet');
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

for (const id of manifest.pets) {
  const directory = new URL(`${id}/`, root);
  const meta = JSON.parse(await readFile(new URL('pet.json', directory), 'utf8'));
  if (meta.id !== id) throw new Error(`${id}: pet.json id mismatch`);
  const sheetPath = meta.spritesheetPath ?? 'spritesheet.webp';
  const bytes = await readFile(new URL(sheetPath, directory));
  if (id === 'solu') {
    const { width, height } = pngSize(bytes);
    const expectedWidth = meta.atlas.cellW * meta.atlas.cols;
    const expectedHeight = meta.atlas.cellH * meta.atlas.rows;
    if (width !== expectedWidth || height !== expectedHeight) {
      throw new Error(`solu: atlas is ${width}x${height}, expected ${expectedWidth}x${expectedHeight}`);
    }
    for (const state of requiredSoluStates) {
      const animation = meta.animations[state];
      if (!animation) throw new Error(`solu: missing required state ${state}`);
      const endCol = (animation.offsetCol ?? 0) + animation.frames;
      if (animation.row < 0 || animation.row >= meta.atlas.rows || endCol > meta.atlas.cols) {
        throw new Error(`solu: state ${state} exceeds atlas bounds`);
      }
    }
  }
}

for (const id of manifest.upcoming ?? []) {
  if (manifest.pets.includes(id)) throw new Error(`${id}: cannot be both selectable and upcoming`);
}

console.log(`Validated ${manifest.pets.length} selectable pet assets.`);
