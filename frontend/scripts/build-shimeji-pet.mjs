#!/usr/bin/env node
// Build a sprite-pet bundle from a ShimejiEE-style mascot folder.
//
// Usage:
//   node scripts/build-shimeji-pet.mjs <shimeji-src-dir> <output-pet-dir> [--phase 1|2]
//
// Example:
//   node scripts/build-shimeji-pet.mjs \
//     "/Users/koschei/Project/芝麻球" \
//     "public/assets/builtin/shimeji-bola" \
//     --phase 2
//
// Phase 1: 1024×512 atlas with 4 rows (idle / walk / spinhead / pinched).
// Phase 2: 1024×1024 atlas extended with 4 more rows for physics animations
//          (grab-wall / climb-wall / climb-ceiling / falling+jumping+bouncing).
//
// Why hardcoded frame sequences instead of fully parsing actions.xml:
// - SitAndSpinHeadAction is a 32-frame loop in the source XML. We need
//   one concise 8-frame review cycle, so we extract just the first head
//   spin (frames 0-7 of the XML loop).
// - Pinched is a conditional (cursor-X-driven) action in the XML; we
//   flatten it into a fixed 6-frame sequence (shime5-10) for our linear
//   sprite player.
// - actions.xml is still parsed for sanity-checking that the referenced
//   frames exist, but the canonical sequences below are authoritative.

import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { join, basename, resolve } from 'node:path'
import sharp from 'sharp'
import { XMLParser } from 'fast-xml-parser'

const CELL = 128
const ATLAS_COLS = 8

// Frame sequences keyed by row, captured by index (1-based shimeN.png).
// Phase 1 rows.
const PHASE1_ROWS = [
  // row 0: idle — single sit pose
  { name: 'idle', frames: [11] },
  // row 1: walking cycle (matches Walk in actions.xml)
  { name: 'walk', frames: [1, 2, 1, 3] },
  // row 2: spin head review animation (first head-spin from
  // SitAndSpinHeadAction, 8 frames)
  { name: 'spinhead', frames: [15, 16, 17, 16, 15, 26, 27, 26] },
  // row 3: pinched/dragged (XML is conditional; flatten to 6 frames)
  { name: 'pinched', frames: [5, 6, 7, 8, 9, 10] },
]

// Phase 2 extra rows (physics animations).
const PHASE2_EXTRA_ROWS = [
  // row 4: grab-wall — single static
  { name: 'grab-wall', frames: [13] },
  // row 5: climb-wall — 4-frame loop derived from XML upward variant
  { name: 'climb-wall', frames: [12, 13, 14, 13] },
  // row 6: climb-ceiling — 4-frame loop
  { name: 'climb-ceiling', frames: [25, 23, 24, 23] },
  // row 7: misc physics frames packed into a single row.
  // col 0: falling (shime4), col 1: jumping (shime22),
  // cols 2-3: bouncing (shime18, shime19)
  { name: 'physics-misc', frames: [4, 22, 18, 19] },
]

function parseArgs(argv) {
  const args = argv.slice(2)
  let phase = 1
  const positional = []
  for (let i = 0; i < args.length; i++) {
    const a = args[i]
    if (a === '--phase') {
      phase = parseInt(args[++i] ?? '1', 10)
      if (phase !== 1 && phase !== 2) {
        throw new Error('--phase must be 1 or 2')
      }
    } else {
      positional.push(a)
    }
  }
  if (positional.length < 2) {
    console.error(
      'Usage: node build-shimeji-pet.mjs <src-dir> <output-pet-dir> [--phase 1|2]',
    )
    process.exit(1)
  }
  return { srcDir: positional[0], outDir: positional[1], phase }
}

async function findFramesDir(srcDir) {
  // Prefer `<src>/img/<mascotName>/shime*.png` (canonical Shimeji-ee
  // layout). Fall back to any subdir of `<src>/img/` that contains
  // shime1.png.
  const imgRoot = join(srcDir, 'img')
  if (!existsSync(imgRoot)) {
    // Some packs put shime*.png at the root; try that too.
    if (existsSync(join(srcDir, 'shime1.png'))) return srcDir
    throw new Error(`Cannot find img/ folder in ${srcDir}`)
  }
  const entries = await readdir(imgRoot, { withFileTypes: true })
  for (const e of entries) {
    if (!e.isDirectory()) continue
    const candidate = join(imgRoot, e.name)
    if (existsSync(join(candidate, 'shime1.png'))) return candidate
  }
  // Maybe sprites are loose in img/ itself.
  if (existsSync(join(imgRoot, 'shime1.png'))) return imgRoot
  throw new Error(`Cannot find shime1.png under ${imgRoot}`)
}

async function loadFrame(framesDir, idx) {
  const path = join(framesDir, `shime${idx}.png`)
  if (!existsSync(path)) {
    throw new Error(`Missing frame: ${path}`)
  }
  const buf = await readFile(path)
  // Force resize to CELL×CELL even though source is already 128×128 — this
  // catches future packs that might use a different cell size and gives
  // us a single canonical output cell dimension.
  const resized = await sharp(buf)
    .resize(CELL, CELL, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .ensureAlpha()
    .png()
    .toBuffer()
  return resized
}

async function buildAtlas(framesDir, rows) {
  const totalRows = rows.length
  const atlasW = CELL * ATLAS_COLS
  const atlasH = CELL * totalRows
  const composites = []
  for (let r = 0; r < rows.length; r++) {
    const row = rows[r]
    if (row.frames.length > ATLAS_COLS) {
      throw new Error(`Row ${row.name} has ${row.frames.length} frames > ${ATLAS_COLS} cols`)
    }
    for (let c = 0; c < row.frames.length; c++) {
      const idx = row.frames[c]
      const frameBuf = await loadFrame(framesDir, idx)
      composites.push({
        input: frameBuf,
        left: c * CELL,
        top: r * CELL,
      })
    }
  }
  return sharp({
    create: {
      width: atlasW,
      height: atlasH,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite(composites)
    .webp({ quality: 95, alphaQuality: 100, lossless: false })
}

function buildPetJson(rows, phase) {
  const animations = {}
  // Row 0: idle
  animations.idle = { row: 0, frames: rows[0].frames.length, fps: 1, loopRestMs: 2000 }
  // Row 1: walking — both running (generic) and run-left share row 1.
  // The shimeji walk frames (shime1/2/3) face LEFT in the source assets
  // (XML uses Velocity="-2,0"), so run-left is the un-flipped baseline
  // and run-right mirrors horizontally via flipX. Swapping the two
  // makes the cat appear to moonwalk.
  animations.running = { row: 1, frames: rows[1].frames.length, fps: 4 }
  animations['run-left'] = { row: 1, frames: rows[1].frames.length, fps: 8 }
  animations['run-right'] = { row: 1, frames: rows[1].frames.length, fps: 8, flipX: true }
  // Row 2: review (SpinHead)
  animations.review = { row: 2, frames: rows[2].frames.length, fps: 12, loopRestMs: 800 }
  // Row 3: waiting (Pinched)
  animations.waiting = { row: 3, frames: rows[3].frames.length, fps: 8, loopRestMs: 600 }

  if (phase === 2) {
    // Row 4: grab-wall (single frame)
    animations['grab-wall'] = { row: 4, frames: 1, fps: 1, loopRestMs: 0 }
    // Row 5: climb-wall — 4 frames
    animations['climb-wall'] = { row: 5, frames: rows[5].frames.length, fps: 8 }
    // Row 6: climb-ceiling — 4 frames
    animations['climb-ceiling'] = { row: 6, frames: rows[6].frames.length, fps: 8 }
    // Row 7 packed: falling (col 0), jumping (col 1), bouncing (cols 2–3)
    // Each gets its own animation with offsetCol so SpritePet can sample
    // a sub-range of the row. Schema field `offsetCol` is honoured by
    // SpritePet (defaults to 0 when absent for backward compatibility).
    animations.falling = { row: 7, frames: 1, fps: 1, offsetCol: 0 }
    animations.jumping = { row: 7, frames: 1, fps: 1, offsetCol: 1 }
    animations.bouncing = { row: 7, frames: 2, fps: 8, offsetCol: 2 }
  }

  return {
    id: 'shimeji-bola',
    displayName: '芝麻球',
    description: 'ShimejiEE 像素猫，移植自 group-finity (BSD/zlib 授权)。原作者 Yuki Yamada / 增强版作者 Kilkakon。',
    spritesheetPath: 'spritesheet.webp',
    kind: 'creature',
    schemaVersion: 2,
    atlas: { cellW: CELL, cellH: CELL, cols: ATLAS_COLS, rows: rows.length },
    animations,
    stateMap: {
      idle: 'idle',
      working: 'run-right',
      compacting: 'review',
      waiting: 'waiting',
    },
    oneShot: [],
    physics: { enabled: phase === 2 },
  }
}

async function sanityCheckXml(srcDir) {
  // Best-effort parse to confirm the source folder is actually Shimeji.
  // Failure here is non-fatal — we still build from the canonical
  // sequences above.
  const xmlPath = join(srcDir, 'conf', 'actions.xml')
  if (!existsSync(xmlPath)) {
    console.warn(`[shimeji] no conf/actions.xml at ${xmlPath} — skipping sanity check`)
    return
  }
  try {
    const xml = await readFile(xmlPath, 'utf8')
    const parser = new XMLParser({ ignoreAttributes: false })
    const tree = parser.parse(xml)
    if (!tree?.Mascot?.ActionList) {
      console.warn('[shimeji] actions.xml missing <Mascot><ActionList>')
    }
  } catch (e) {
    console.warn('[shimeji] actions.xml parse warning:', e.message)
  }
}

async function main() {
  const { srcDir, outDir, phase } = parseArgs(process.argv)
  const srcAbs = resolve(srcDir)
  const outAbs = resolve(outDir)

  console.log(`[shimeji] src=${srcAbs}`)
  console.log(`[shimeji] out=${outAbs}`)
  console.log(`[shimeji] phase=${phase}`)

  await sanityCheckXml(srcAbs)
  const framesDir = await findFramesDir(srcAbs)
  console.log(`[shimeji] frames=${framesDir}`)

  const rows = phase === 2 ? [...PHASE1_ROWS, ...PHASE2_EXTRA_ROWS] : PHASE1_ROWS
  const atlas = await buildAtlas(framesDir, rows)

  await mkdir(outAbs, { recursive: true })
  const webpPath = join(outAbs, 'spritesheet.webp')
  await atlas.toFile(webpPath)
  console.log(`[shimeji] wrote ${webpPath}`)

  const petJson = buildPetJson(rows, phase)
  const jsonPath = join(outAbs, 'pet.json')
  await writeFile(jsonPath, JSON.stringify(petJson, null, 2) + '\n')
  console.log(`[shimeji] wrote ${jsonPath}`)

  console.log(`[shimeji] done — atlas size: ${ATLAS_COLS * CELL}x${rows.length * CELL}`)
}

main().catch((e) => {
  console.error('[shimeji] FAILED:', e)
  process.exit(1)
})
