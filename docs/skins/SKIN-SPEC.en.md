# PawBae Skin Spec (v1)

A PawBae skin is a folder with one spritesheet and one `pet.json`. Install it in-app via **Skin Workshop 🎨 → Import folder**; re-importing the same id upgrades it in place.

**Lowest bar: a single image.** Pick **Import image** and any PNG/WebP/JPG/GIF becomes a standing skin (whole image = a 1-frame `idle`); every other animation falls back gracefully. Upgrade to a real atlas later using this spec.

## Folder layout

```
my-skin/
├── pet.json          # manifest (required)
└── spritesheet.webp  # sprite atlas (required; png/webp/jpg all fine)
```

## Atlas format

- One image cut into a **uniform grid**: `cols` × `rows` cells of `cellW` × `cellH` pixels.
- **One row = one animation**, played left to right across `frames` cells.
- Transparent-background WebP recommended; set `imageRendering: "pixelated"` for pixel art.
- Keep the image within 4096×4096.

## The standard 9-row layout (recommended)

Omitting `animations` applies this layout with fixed 192×208 cells, 8 cols × 9 rows (a 1536×1872 image):

| Row | Name | Frames | Default fps | Used for |
|---|---|---|---|---|
| 0 | `idle` | 6 | 2 | standing (required) |
| 1 | `run-right` | 8 | 8 | running right (stroll / coming home) |
| 2 | `run-left` | 8 | 8 | running left (stroll / departing) |
| 3 | `waving` | 4 | 12 | greeting |
| 4 | `jumping` | 5 | 6 | jump (one-shot) |
| 5 | `failed` | 8 | 12 | task failed |
| 6 | `waiting` | 6 | 6 | waiting for your approval (600ms loop rest) |
| 7 | `running` | 6 | 6 | agent working |
| 8 | `review` | 6 | 12 | reviewing work |

## pet.json reference

```jsonc
{
  "id": "my-skin",            // required; install folder name, safe chars (unicode ok; no / \ : .. , no leading dot, ≤64 chars)
  "displayName": "Cloudlet",  // display name (users can still nickname it)
  "description": "One-liner",
  "spritesheetPath": "spritesheet.webp",  // defaults to spritesheet.webp
  "imageRendering": "auto",   // auto | pixelated | crisp-edges
  "atlas": { "cellW": 192, "cellH": 208, "cols": 8, "rows": 9 },
  "animations": {             // omit entirely = inherit the standard 9 rows
    "idle": { "row": 0, "frames": 6, "fps": 2 }
    // row required; frames defaults to 1; optional fps, loopRestMs,
    // flipX, offsetCol (start column), displayScale (per-row scale)
  },
  "stateMap": {               // agent state → row name (defaults below)
    "idle": "idle", "working": "running", "compacting": "running", "waiting": "waiting"
  },
  "oneShot": ["jumping"],     // rows that play once and hold (default ["jumping"])
  "physics": { "enabled": true }  // opt into climb/throw physics rows
}
```

## Advanced vocabulary (optional; each row unlocks a behavior)

The engine looks rows up by name and falls back when missing — **it never errors**:

- **Physics** (drag/throw/climb): `falling`, `bouncing`, `grab-wall`, `grab-wall-flipped`, `climb-wall`, `climb-wall-flipped`, `climb-ceiling`, `climb-ceiling-flipped`
- **Meals**: `eat` (falls back to `happy`, then to nothing)
- **Input reactions**: `react-keyboard`, `react-mouse`
- **Idle micro-actions** (declaring one adds it to the random pool): `blink`, `happy`, `thinking`, `pounce`, `yawn`, `sleep`, `rest`, `dance`, `spin`, `peek`
- **Voice emotions**: `happy`, `sleep`, `eat`, `angry`
- **Task outcomes** (one-shot): `done-success`, `done-fail`

Full-featured reference: the builtin Yoonie (`apps/desktop/public/assets/builtin/yoonie/`, 31 rows).

## Import validation (generous in, strict out)

**Blocks the import**: invalid JSON, traversal characters in the id or sheet path, non-positive atlas fields, atlas larger than the actual image, declared animations without `idle`, out-of-range rows/frames.

**Warns only**: image not evenly divided, missing standard rows, animation names that look like typos, fps > 60, very large images.

## Distribution

v1 skins travel as folders/zips (unzip, then Import folder). "Open skins folder" in the workshop jumps straight to the install dir (`~/.codex/pets`, layout-compatible with the codex CLI hatch-pet skill).
