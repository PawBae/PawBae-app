# 官方四宠资产生产管线 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立「策展图 → 抠图 → 对齐打包 → pet.json → 桌面验证」的官方四宠(Solu/Muru/Riffi/Luma)资产生产管线,零引擎改动。

**Architecture:** 纯新增:`tools/pet-pipeline/` 下的 Python 工具链(几何/打包纯函数 + CLI)、`docs/art/` 下的风格圣经与生产手册。产物落入现有 `public/assets/builtin/<id>/`(spritesheet.webp + pet.json),由现有 SpritePet 引擎直接播放。

**Tech Stack:** Python 3.11+(Pillow、pytest;可选 rembg 抠图)、现有 Svelte 5 + Tauri 2 App 仅作验证载体。

## Global Constraints

- **零引擎改动**:不修改 `src/` 下任何文件;只新增 `tools/pet-pipeline/`、`docs/art/`、`public/assets/builtin/<id>/`,以及向 `public/assets/builtin/pets-manifest.json` 的 `pets` 数组追加条目
- **pet.json 契约**(源自 [codex-pet.ts](../../src/lib/utils/codex-pet.ts) 与 shimeji-bola 实例):`schemaVersion: 2`;`atlas: {cellW, cellH, cols, rows}`;`animations: {<state>: {row, frames, fps?, loopRestMs?, flipX?}}`;`stateMap` 仅四键 `idle/working/compacting/waiting`;`oneShot` 为字符串数组;`imageRendering: "auto"`(非像素风)
- **引擎回落规则**:未声明的动画状态回落 `idle`,故 idle 行是唯一硬性要求;喂食系统自动探测 `eat`/`happy` 行
- **朝向约定**:所有 running 帧画面朝左;`run-right` 由 `flipX: true` 生成
- **单元格**:512×512;帧内角色底部中心锚定,pad_bottom 24px;同一宠物所有帧共用一个缩放系数(防止帧间大小跳变)
- **工作流**:禁止直推 main;在 `feat/official-pets-pipeline` 分支工作,PR 经 `gh pr create`
- **并行会话协调**:另一会话(task_58cadb96)正在改 `pets-manifest.json`(下架版权皮肤)。凡改 manifest 的步骤,先 `git pull --rebase` 或确认该会话 PR 状态,冲突以"版权条目删除 + 四宠条目追加"合并
- **生成素材不入库**:`tools/pet-pipeline/input/`(策展工作区)与 `.venv` 加 gitignore;创作过程记录(prompt、迭代稿)存团队盘,不进 git

## File Structure

```
tools/pet-pipeline/
  .gitignore               # input/ .venv/ __pycache__/
  requirements.txt         # pillow, pytest
  requirements-cutout.txt  # rembg[cpu](重依赖,单独安装)
  petpipe/
    __init__.py
    geometry.py            # alpha_bbox / fit_scale / place_frame(纯函数)
    packer.py              # STATE_SPECS / collect_frames / build_pet
    cutout.py              # rembg 封装(人工核验工具)
    cli.py                 # pack / contact-sheet 子命令
    __main__.py
  tests/
    test_geometry.py
    test_packer.py
    test_cli.py
  input/                   # gitignored:<pet-id>/<state>/*.png + pet.meta.json
docs/art/
  pet-style-bible.md       # 风格圣经 + 四宠角色卡 + prompt 模板
  pet-production-runbook.md# 生产手册 + QA 清单
public/assets/builtin/
  solu/ muru/ riffi/ luma/ # 资产产出(spritesheet.webp + pet.json)
```

---

### Task 1: 管线脚手架 + geometry 模块

**Files:**
- Create: `tools/pet-pipeline/.gitignore`
- Create: `tools/pet-pipeline/requirements.txt`
- Create: `tools/pet-pipeline/requirements-cutout.txt`
- Create: `tools/pet-pipeline/petpipe/__init__.py`(空文件)
- Create: `tools/pet-pipeline/petpipe/geometry.py`
- Test: `tools/pet-pipeline/tests/test_geometry.py`

**Interfaces:**
- Produces:
  - `alpha_bbox(img: Image.Image) -> tuple[int, int, int, int]` — RGBA 图的 alpha 包围盒 (l, t, r, b);全透明抛 `ValueError`
  - `fit_scale(bbox_sizes: list[tuple[int, int]], cell: tuple[int, int], margin: float = 0.08) -> float` — 使最大帧适配单元格的统一缩放系数,横向留 2×margin、纵向留 1×margin,**永不放大**(上限 1.0)
  - `place_frame(img: Image.Image, scale: float, cell: tuple[int, int], pad_bottom: int = 24) -> Image.Image` — 裁剪→缩放→底部中心锚定,返回 cell 尺寸的 RGBA 画布

- [ ] **Step 1: 建目录与环境**

```bash
mkdir -p tools/pet-pipeline/petpipe tools/pet-pipeline/tests tools/pet-pipeline/input
cd tools/pet-pipeline
printf 'input/\n.venv/\n__pycache__/\n*.pyc\n' > .gitignore
printf 'pillow>=10.0\npytest>=8.0\n' > requirements.txt
printf 'rembg[cpu]>=2.0\n' > requirements-cutout.txt
touch petpipe/__init__.py
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
```

Expected: pip 安装 pillow、pytest 成功。

- [ ] **Step 2: 写失败测试**

`tools/pet-pipeline/tests/test_geometry.py`:

```python
from PIL import Image
from petpipe.geometry import alpha_bbox, fit_scale, place_frame


def make_blob(canvas=(100, 100), rect=(20, 30, 60, 80)):
    """在透明画布 rect 处放一个不透明红色矩形。"""
    img = Image.new("RGBA", canvas, (0, 0, 0, 0))
    block = Image.new(
        "RGBA", (rect[2] - rect[0], rect[3] - rect[1]), (255, 0, 0, 255)
    )
    img.paste(block, rect[:2])
    return img


def test_alpha_bbox():
    assert alpha_bbox(make_blob()) == (20, 30, 60, 80)


def test_alpha_bbox_rejects_empty():
    import pytest

    with pytest.raises(ValueError):
        alpha_bbox(Image.new("RGBA", (10, 10), (0, 0, 0, 0)))


def test_fit_scale_downscales_to_fit():
    s = fit_scale([(1000, 800)], (512, 512), margin=0.08)
    # 横向可用 512*0.84=430.08 是瓶颈:430.08/1000
    assert abs(s - 0.43008) < 1e-6


def test_fit_scale_never_upscales():
    assert fit_scale([(50, 50)], (512, 512)) == 1.0


def test_place_frame_bottom_center():
    img = make_blob((100, 100), (20, 30, 60, 80))  # 40x50 内容
    cell = place_frame(img, 1.0, (512, 512), pad_bottom=24)
    assert cell.size == (512, 512)
    # x = (512-40)//2 = 236, y = 512-24-50 = 438
    assert cell.getchannel("A").getbbox() == (236, 438, 276, 488)
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/test_geometry.py -v`
Expected: FAIL / ERROR,`ModuleNotFoundError: No module named 'petpipe.geometry'`

- [ ] **Step 4: 实现 geometry.py**

`tools/pet-pipeline/petpipe/geometry.py`:

```python
"""帧几何:包围盒、统一缩放、底部中心锚定。全部纯函数。"""

from PIL import Image


def alpha_bbox(img: Image.Image) -> tuple[int, int, int, int]:
    if img.mode != "RGBA":
        raise ValueError(f"expected RGBA, got {img.mode}")
    bbox = img.getchannel("A").getbbox()
    if bbox is None:
        raise ValueError("fully transparent frame")
    return bbox


def fit_scale(
    bbox_sizes: list[tuple[int, int]],
    cell: tuple[int, int],
    margin: float = 0.08,
) -> float:
    cw, ch = cell
    avail_w = cw * (1 - 2 * margin)
    avail_h = ch * (1 - margin)
    max_w = max(w for w, _ in bbox_sizes)
    max_h = max(h for _, h in bbox_sizes)
    return min(avail_w / max_w, avail_h / max_h, 1.0)


def place_frame(
    img: Image.Image,
    scale: float,
    cell: tuple[int, int],
    pad_bottom: int = 24,
) -> Image.Image:
    crop = img.crop(alpha_bbox(img))
    w, h = crop.size
    nw, nh = max(1, round(w * scale)), max(1, round(h * scale))
    if (nw, nh) != (w, h):
        crop = crop.resize((nw, nh), Image.LANCZOS)
    cw, ch = cell
    canvas = Image.new("RGBA", cell, (0, 0, 0, 0))
    canvas.paste(crop, ((cw - nw) // 2, max(0, ch - pad_bottom - nh)))
    return canvas
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/test_geometry.py -v`
Expected: 5 passed

- [ ] **Step 6: Commit**

```bash
git add tools/pet-pipeline
git commit -m "feat(pet-pipeline): 管线脚手架 + 帧几何模块(bbox/缩放/锚定)"
```

---

### Task 2: 图集打包器 + pet.json 生成

**Files:**
- Create: `tools/pet-pipeline/petpipe/packer.py`
- Test: `tools/pet-pipeline/tests/test_packer.py`

**Interfaces:**
- Consumes: `geometry.alpha_bbox / fit_scale / place_frame`(签名见 Task 1)
- Produces:
  - `STATE_SPECS: dict[str, StateSpec]` — 状态注册表,插入顺序即图集行顺序;`StateSpec(fps: int, loop_rest_ms: int | None, one_shot: bool)`
  - `collect_frames(input_dir: Path) -> dict[str, list[Path]]` — 扫描 `<input_dir>/<state>/*.png`(排序),缺 idle 抛 `ValueError`
  - `build_pet(input_dir: Path) -> tuple[Image.Image, dict]` — 返回 (拼合图集, pet.json dict);读取 `<input_dir>/pet.meta.json` 取 id/displayName/description

- [ ] **Step 1: 写失败测试**

`tools/pet-pipeline/tests/test_packer.py`:

```python
import json

from PIL import Image
from petpipe.packer import STATE_SPECS, build_pet, collect_frames


def make_input(tmp_path, states={"idle": 1, "running": 2}):
    (tmp_path / "pet.meta.json").write_text(
        json.dumps(
            {"id": "solu", "displayName": "小煦", "description": "太阳系。"}
        ),
        encoding="utf-8",
    )
    for state, n in states.items():
        d = tmp_path / state
        d.mkdir()
        for i in range(n):
            img = Image.new("RGBA", (200, 200), (0, 0, 0, 0))
            img.paste(Image.new("RGBA", (80, 100), (0, 200, 0, 255)), (60, 80))
            img.save(d / f"frame{i:02d}.png")
    return tmp_path


def test_collect_frames_requires_idle(tmp_path):
    import pytest

    make_input(tmp_path, states={"running": 2})
    with pytest.raises(ValueError):
        collect_frames(tmp_path)


def test_build_pet_atlas_and_json(tmp_path):
    sheet, pet = build_pet(make_input(tmp_path))
    assert pet["atlas"] == {"cellW": 512, "cellH": 512, "cols": 2, "rows": 2}
    assert sheet.size == (1024, 1024)
    assert pet["schemaVersion"] == 2
    assert pet["imageRendering"] == "auto"
    assert pet["spritesheetPath"] == "spritesheet.webp"
    assert pet["animations"]["idle"] == {
        "row": 0, "frames": 1, "fps": 2, "loopRestMs": 2000
    }
    assert pet["animations"]["running"]["row"] == 1
    # running 别名:run-left 同行不翻转(源图朝左),run-right 翻转
    assert pet["animations"]["run-left"]["row"] == 1
    assert pet["animations"]["run-right"]["flipX"] is True
    # 缺 review/waiting 时 stateMap 回落 idle
    assert pet["stateMap"] == {
        "idle": "idle", "working": "running",
        "compacting": "idle", "waiting": "idle",
    }
    assert pet["oneShot"] == []  # 本输入无 one-shot 状态


def test_build_pet_paints_cells(tmp_path):
    sheet, _ = build_pet(make_input(tmp_path))
    # idle 行第 0 格有内容,第 1 格(空槽)全透明
    assert sheet.crop((0, 0, 512, 512)).getchannel("A").getbbox() is not None
    assert sheet.crop((512, 0, 1024, 512)).getchannel("A").getbbox() is None


def test_state_specs_defines_all_planned_states():
    assert list(STATE_SPECS) == [
        "idle", "running", "waving", "happy", "eat",
        "failed", "waiting", "review", "jumping",
    ]
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/test_packer.py -v`
Expected: ERROR,`No module named 'petpipe.packer'`

- [ ] **Step 3: 实现 packer.py**

`tools/pet-pipeline/petpipe/packer.py`:

```python
"""图集打包:输入目录(状态子目录装帧 PNG)→ (spritesheet, pet.json dict)。"""

import json
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

from .geometry import alpha_bbox, fit_scale, place_frame

CELL = (512, 512)
PAD_BOTTOM = 24


@dataclass(frozen=True)
class StateSpec:
    fps: int
    loop_rest_ms: int | None = None
    one_shot: bool = False


# 插入顺序 = 图集行顺序。缺失状态自动跳过(引擎回落 idle)。
STATE_SPECS: dict[str, StateSpec] = {
    "idle": StateSpec(fps=2, loop_rest_ms=2000),
    "running": StateSpec(fps=6),
    "waving": StateSpec(fps=3, one_shot=True),
    "happy": StateSpec(fps=4),
    "eat": StateSpec(fps=4, one_shot=True),
    "failed": StateSpec(fps=2, loop_rest_ms=1000),
    "waiting": StateSpec(fps=2, loop_rest_ms=800),
    "review": StateSpec(fps=3),
    "jumping": StateSpec(fps=1, one_shot=True),
}


def collect_frames(input_dir: Path) -> dict[str, list[Path]]:
    frames: dict[str, list[Path]] = {}
    for state in STATE_SPECS:
        d = input_dir / state
        if d.is_dir():
            paths = sorted(p for p in d.iterdir() if p.suffix.lower() == ".png")
            if paths:
                frames[state] = paths
    if "idle" not in frames:
        raise ValueError(f"{input_dir}: idle 状态帧是硬性要求")
    return frames


def build_pet(input_dir: Path) -> tuple[Image.Image, dict]:
    meta = json.loads((input_dir / "pet.meta.json").read_text(encoding="utf-8"))
    frame_paths = collect_frames(input_dir)
    images = {
        s: [Image.open(p).convert("RGBA") for p in ps]
        for s, ps in frame_paths.items()
    }

    sizes = []
    for imgs in images.values():
        for im in imgs:
            left, top, right, bottom = alpha_bbox(im)
            sizes.append((right - left, bottom - top))
    scale = fit_scale(sizes, CELL)

    states = list(images)
    cols = max(len(v) for v in images.values())
    rows = len(states)
    sheet = Image.new("RGBA", (CELL[0] * cols, CELL[1] * rows), (0, 0, 0, 0))

    animations: dict[str, dict] = {}
    for row, state in enumerate(states):
        for col, im in enumerate(images[state]):
            sheet.paste(
                place_frame(im, scale, CELL, PAD_BOTTOM),
                (col * CELL[0], row * CELL[1]),
            )
        spec = STATE_SPECS[state]
        anim: dict = {"row": row, "frames": len(images[state]), "fps": spec.fps}
        if spec.loop_rest_ms is not None:
            anim["loopRestMs"] = spec.loop_rest_ms
        animations[state] = anim

    if "running" in animations:
        base = animations["running"]
        animations["run-left"] = {
            "row": base["row"], "frames": base["frames"], "fps": 8
        }
        animations["run-right"] = {
            "row": base["row"], "frames": base["frames"], "fps": 8, "flipX": True
        }

    def fallback(state: str) -> str:
        return state if state in frame_paths else "idle"

    pet_json = {
        "id": meta["id"],
        "displayName": meta["displayName"],
        "description": meta.get("description", ""),
        "spritesheetPath": "spritesheet.webp",
        "imageRendering": "auto",
        "kind": "creature",
        "schemaVersion": 2,
        "atlas": {"cellW": CELL[0], "cellH": CELL[1], "cols": cols, "rows": rows},
        "animations": animations,
        "stateMap": {
            "idle": "idle",
            "working": fallback("running"),
            "compacting": fallback("review"),
            "waiting": fallback("waiting"),
        },
        "oneShot": [s for s in states if STATE_SPECS[s].one_shot],
        "physics": {"enabled": True},
    }
    return sheet, pet_json
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/ -v`
Expected: 9 passed(geometry 5 + packer 4)

- [ ] **Step 5: Commit**

```bash
git add tools/pet-pipeline
git commit -m "feat(pet-pipeline): 图集打包器 + pet.json 生成(状态表/别名/回落)"
```

---

### Task 3: CLI(pack / contact-sheet)

**Files:**
- Create: `tools/pet-pipeline/petpipe/cli.py`
- Create: `tools/pet-pipeline/petpipe/__main__.py`
- Test: `tools/pet-pipeline/tests/test_cli.py`

**Interfaces:**
- Consumes: `packer.build_pet(input_dir) -> (Image, dict)`
- Produces:
  - CLI `python -m petpipe pack <input_dir> --out <dir> [--dry-run]` — 写出 `<out>/<id>/spritesheet.webp` + `pet.json`
  - CLI `python -m petpipe contact-sheet <dir> --out <png>` — 目录内所有 PNG 拼成带文件名标注的策展预览网格
  - `main(argv: list[str] | None = None) -> None`(供测试直接调用)

- [ ] **Step 1: 写失败测试**

`tools/pet-pipeline/tests/test_cli.py`:

```python
import json

from PIL import Image
from petpipe.cli import main
from tests.test_packer import make_input


def test_pack_writes_outputs(tmp_path):
    inp = tmp_path / "in"
    inp.mkdir()
    make_input(inp)
    out = tmp_path / "out"
    main(["pack", str(inp), "--out", str(out)])
    pet = json.loads((out / "solu" / "pet.json").read_text(encoding="utf-8"))
    assert pet["id"] == "solu"
    sheet = Image.open(out / "solu" / "spritesheet.webp")
    assert sheet.size == (1024, 1024)


def test_contact_sheet(tmp_path):
    d = tmp_path / "imgs"
    d.mkdir()
    for i in range(3):
        Image.new("RGBA", (64, 64), (255, 0, 0, 255)).save(d / f"c{i}.png")
    out = tmp_path / "sheet.png"
    main(["contact-sheet", str(d), "--out", str(out)])
    assert out.exists()
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/test_cli.py -v`
Expected: ERROR,`No module named 'petpipe.cli'`

- [ ] **Step 3: 实现 cli.py 与 __main__.py**

`tools/pet-pipeline/petpipe/cli.py`:

```python
"""petpipe 命令行:pack(打包宠物)与 contact-sheet(策展预览)。"""

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw

from .packer import build_pet


def cmd_pack(args: argparse.Namespace) -> None:
    sheet, pet = build_pet(Path(args.input_dir))
    if args.dry_run:
        summary = {k: pet[k] for k in ("id", "atlas", "stateMap", "oneShot")}
        print(json.dumps(summary, ensure_ascii=False, indent=2))
        print("animations:", ", ".join(pet["animations"]))
        return
    out_dir = Path(args.out) / pet["id"]
    out_dir.mkdir(parents=True, exist_ok=True)
    sheet.save(out_dir / "spritesheet.webp", quality=90, method=6)
    (out_dir / "pet.json").write_text(
        json.dumps(pet, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {out_dir} (sheet {sheet.width}x{sheet.height})")


def cmd_contact_sheet(args: argparse.Namespace) -> None:
    paths = sorted(Path(args.dir).glob("*.png"))
    if not paths:
        raise SystemExit(f"{args.dir}: 没有 PNG")
    cell, label_h, cols = 256, 20, 6
    rows = -(-len(paths) // cols)
    sheet = Image.new(
        "RGB", (cell * cols, (cell + label_h) * rows), (240, 240, 240)
    )
    draw = ImageDraw.Draw(sheet)
    for i, p in enumerate(paths):
        img = Image.open(p).convert("RGBA")
        img.thumbnail((cell, cell))
        x, y = (i % cols) * cell, (i // cols) * (cell + label_h)
        sheet.paste(img, (x, y), img)
        draw.text((x + 4, y + cell + 2), p.name, fill=(60, 60, 60))
    sheet.save(args.out)
    print(f"wrote {args.out} ({len(paths)} images)")


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="petpipe")
    sub = parser.add_subparsers(dest="command", required=True)

    p_pack = sub.add_parser("pack", help="打包宠物资产")
    p_pack.add_argument("input_dir")
    p_pack.add_argument("--out", required=True)
    p_pack.add_argument("--dry-run", action="store_true")
    p_pack.set_defaults(func=cmd_pack)

    p_cs = sub.add_parser("contact-sheet", help="生成策展预览网格")
    p_cs.add_argument("dir")
    p_cs.add_argument("--out", required=True)
    p_cs.set_defaults(func=cmd_contact_sheet)

    args = parser.parse_args(argv)
    args.func(args)
```

`tools/pet-pipeline/petpipe/__main__.py`:

```python
from .cli import main

main()
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/ -v`
Expected: 11 passed

- [ ] **Step 5: Commit**

```bash
git add tools/pet-pipeline
git commit -m "feat(pet-pipeline): CLI pack/contact-sheet 子命令"
```

---

### Task 4: 抠图工具(rembg 封装,人工核验)

**Files:**
- Create: `tools/pet-pipeline/petpipe/cutout.py`
- Modify: `tools/pet-pipeline/petpipe/cli.py`(追加 cutout 子命令)

**Interfaces:**
- Consumes: 无(独立模块;rembg 为可选依赖)
- Produces:
  - `cutout_dir(src: Path, dst: Path, model: str = "birefnet-general") -> int` — 对 src 下每张 png/jpg/webp 去底,输出同名 PNG 到 dst,返回处理张数;rembg 未安装时抛 `SystemExit` 并提示安装命令
  - CLI `python -m petpipe cutout <src> --out <dst> [--model birefnet-general]`

抠图质量只能人工核验(绒毛边缘),不写自动断言;测试只覆盖"未安装依赖时报错清晰"。

- [ ] **Step 1: 实现 cutout.py**

`tools/pet-pipeline/petpipe/cutout.py`:

```python
"""策展图去底。绒毛边缘质量必须人工核验(配合 contact-sheet)。"""

from pathlib import Path

SUFFIXES = {".png", ".jpg", ".jpeg", ".webp"}


def cutout_dir(src: Path, dst: Path, model: str = "birefnet-general") -> int:
    try:
        from rembg import new_session, remove
    except ImportError:
        raise SystemExit(
            "rembg 未安装。运行:.venv/bin/pip install -r requirements-cutout.txt"
        )
    session = new_session(model)
    dst.mkdir(parents=True, exist_ok=True)
    count = 0
    for p in sorted(src.iterdir()):
        if p.suffix.lower() not in SUFFIXES:
            continue
        result = remove(p.read_bytes(), session=session)
        (dst / f"{p.stem}.png").write_bytes(result)
        count += 1
    print(f"cutout {count} images -> {dst}")
    return count
```

在 `cli.py` 的 `main()` 中追加(与其他子命令并列):

```python
    p_cut = sub.add_parser("cutout", help="批量去底(需 requirements-cutout)")
    p_cut.add_argument("src")
    p_cut.add_argument("--out", required=True)
    p_cut.add_argument("--model", default="birefnet-general")
    p_cut.set_defaults(
        func=lambda a: __import__(
            "petpipe.cutout", fromlist=["cutout_dir"]
        ).cutout_dir(Path(a.src), Path(a.out), a.model)
    )
```

- [ ] **Step 2: 回归 + 冒烟**

Run: `cd tools/pet-pipeline && .venv/bin/python -m pytest tests/ -v && .venv/bin/python -m petpipe cutout --help`
Expected: 11 passed;cutout 帮助文本正常打印(不装 rembg 也应能打印 help)

- [ ] **Step 3: Commit**

```bash
git add tools/pet-pipeline
git commit -m "feat(pet-pipeline): rembg 抠图子命令(人工核验配套)"
```

---

### Task 5: 风格圣经(docs/art/pet-style-bible.md)

**Files:**
- Create: `docs/art/pet-style-bible.md`

**Interfaces:**
- Produces: 生成阶段唯一的 prompt 事实源;§5 生成管线(v1.1 UGC)将直接复用其主模板

- [ ] **Step 1: 写入文档**(以下为完整正文,直接落盘)

````markdown
# PawBae 宠物风格圣经 v1

> 官方四宠与未来 UGC 生成的统一风格事实源。改动需过 Yining。

## 统一风格 DNA

- **材质**:软羊毛毡/绒布质感(felted wool),短绒可见
- **体型**:胖圆,头身比≈1:1.2,短腿或无腿感,重心低
- **五官**:大而亮的眼睛(高光两点)、腮红、小嘴
- **光**:柔和棚光 + 轻微轮廓光(rim light)
- **禁忌**:写实毛发、锐利爪牙、复杂背景、文字水印
- **朝向**:动作帧统一朝左(run-right 由引擎翻转)

## 主 Prompt 模板(生成底稿用)

```
A single {species_desc}, chubby round creature made of soft felted wool,
pastel {palette} color scheme, big sparkling eyes with two highlights,
blush cheeks, {signature_element}, full body, {pose}, facing left,
clean solid light-gray background, soft studio lighting, gentle rim light,
3D render, kawaii collectible toy style, no text, no watermark
```

变体生成(状态帧)一律用**图像编辑/角色参考模式**,以定稿主立绘为参考图,
指令只描述增量:"same character, identical style and colors, {delta}"。
**每次只改一点**(一个姿势或一个表情),一致性最稳。

## 四宠角色卡

| 字段 | Solu 小煦 | Muru 雾露 | Riffi 雷栗 | Luma 星沫 |
|---|---|---|---|---|
| 元素 | 日 | 雾 | 雷 | 星 |
| species_desc | sun lion cub | mist lop bunny | thunder squirrel-dragon | star cat-sheep |
| signature_element | flower-petal mane in warm gradient | cloud-shaped tail, long droopy ears | lightning-bolt ears and tail | golden crescent horns, stardust tail with heart |
| palette(近似) | #FFD98E / #FFB36B / #F58F5E | #C9D6F5 / #B3C7F0 / #E6E9FA | #BFE8D2 / #A8E0C0 / #F5E39A | #F5AFC8 / #3D4E9E / #E8C86A |
| 性格种子 | 热情阳光 | 害羞治愈 | 元气冒失 | 嗜睡梦幻 |
| 底稿状态 | 主立绘已定稿(2026-07-10 海报) | 同左 | 同左 | 同左 |

## 状态帧 delta 指令表(每状态,逐帧)

| 状态 | 帧数 | delta 指令(逐帧) |
|---|---|---|
| idle | 2 | ① 主立绘站姿 ② same pose, eyes half-closed mid-blink |
| running | 4 | ① crouching, ready to hop ② launching off ground, leaning forward ③ mid-air, limbs tucked ④ landing, slightly squashed |
| waving | 2 | ① one paw raised high waving ② paw at mid-height |
| happy | 2 | ① eyes closed in a big smile, arms up ② same, slightly leaning back |
| eat | 2 | ① holding a small cookie near mouth ② chewing, cheeks puffed |
| failed | 2 | ① drooping posture, teary eyes, ears down ② same, head lower |
| waiting | 2 | ① sitting, looking up expectantly ② same, head tilted |
| review | 2 | ① wearing tiny round glasses, looking down attentively ② same, paw raised to chin |
| jumping | 1 | ① mid-air jump, limbs spread joyfully |

## 策展 QC 清单(每帧过一遍)

- [ ] 记忆点元素在(鬃/云尾/闪电/星角)且形状未漂移
- [ ] 色板在角色卡范围内(允许光影偏移,不允许换色)
- [ ] 头身比与主立绘一致(±10%)
- [ ] 无多余肢体/手指、无背景残留、无文字
- [ ] 朝向正确(动作帧朝左)
- [ ] 脚底/身体底部完整(打包要做底部锚定)

## 可选:角色 LoRA(附录)

单角色合格图 ≥10 张后,可训小 LoRA 提升后续扩展一致性。
官方四宠值得,UGC 不做。训练配置另行记录到团队盘,不入库。
````

- [ ] **Step 2: 检查表格与代码块渲染**

Run: `ls docs/art/ && head -20 docs/art/pet-style-bible.md`
Expected: 文件存在,标题正常。

- [ ] **Step 3: Commit**

```bash
git add docs/art/pet-style-bible.md
git commit -m "docs(art): 宠物风格圣经 v1(四宠角色卡 + prompt 模板 + QC 清单)"
```

---

### Task 6: 生产手册 + Solu 试点接入清单

**Files:**
- Create: `docs/art/pet-production-runbook.md`
- Modify: `public/assets/builtin/pets-manifest.json`(资产就绪后,每宠一条)

**Interfaces:**
- Consumes: Task 3 的 CLI、Task 5 的风格圣经
- Produces: 人机协作生产流程;四宠上线的验收标准

- [ ] **Step 1: 写入生产手册**(完整正文)

````markdown
# 官方宠物生产手册

每只宠物走一遍以下流程(约 2~3 天/只,大头在策展)。

## 0. 前置

- 读 docs/art/pet-style-bible.md
- `cd tools/pet-pipeline && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt`
- 抠图额外:`.venv/bin/pip install -r requirements-cutout.txt`(首次下载模型较慢)

## 1. 生成(人工 + 图像模型)

- 用风格圣经的 delta 指令表,以主立绘为角色参考逐状态生成
- 每帧出 3 个候选;产物存 `input/<pet-id>/_raw/<state>/`
- prompt 与迭代记录存团队盘(确权用),不入 git

## 2. 策展

- `python -m petpipe contact-sheet input/<pet-id>/_raw/<state> --out /tmp/cs.png`
- 按风格圣经 QC 清单选帧,合格帧复制到 `input/<pet-id>/_cut_src/<state>/`,
  按播放顺序命名 `frame00.png, frame01.png...`

## 3. 抠图

- `python -m petpipe cutout input/<pet-id>/_cut_src/<state> --out input/<pet-id>/<state>`
- 再出一张 contact-sheet 核验绒毛边缘;发丝级瑕疵可接受,块状残留重抠或换帧

## 4. 元数据

`input/<pet-id>/pet.meta.json`:

```json
{ "id": "solu", "displayName": "小煦", "description": "太阳系小狮,花瓣鬃毛,热情阳光。" }
```

## 5. 打包

- 预检:`python -m petpipe pack input/<pet-id> --out /tmp --dry-run`
- 正式:`python -m petpipe pack input/<pet-id> --out ../../public/assets/builtin`
- 体积预算:spritesheet.webp ≤ 3MB;超了先降 quality 到 80,再考虑 cellW 384

## 6. manifest 接入

- **先 rebase**(下架版权皮肤的会话也在改这个文件)
- `public/assets/builtin/pets-manifest.json` 的 `pets` 数组追加 `"<pet-id>"`

## 7. 桌面 QA 清单

`pnpm install && pnpm tauri dev`,设置面板切到新宠物,逐项核对:

- [ ] idle:呼吸/眨眼两帧交替,2s 停顿节奏自然
- [ ] 拖拽宠物移动,松手后行为正常
- [ ] agent 工作中(任意会话 processing)→ running 跳步循环,左右朝向正确
- [ ] agent 出错 → failed 委屈状
- [ ] waiting 状态(agent 等待输入)→ 期待坐姿
- [ ] 喂食(现有机制)→ eat 单发动画播完回 idle
- [ ] 缩放窗口/高 DPI 下边缘无明显毛刺(imageRendering auto)
- [ ] 图集加载无 404(devtools network)

## 8. 完成定义(每宠)

- QA 清单全过 + spritesheet ≤3MB + pet.json 过 dry-run
- commit:`feat(assets): 官方宠物 <中文名>(<id>)`
- 四宠齐后与「版权皮肤下架」PR 合并轨道对齐,一起进闭测构建
````

- [ ] **Step 2: Commit 手册**

```bash
git add docs/art/pet-production-runbook.md
git commit -m "docs(art): 官方宠物生产手册(生成→策展→抠图→打包→QA)"
```

- [ ] **Step 3: Solu 试点**

按手册跑通 Solu 全流程(生成与策展为人工环节,与 Claude 协作进行)。产物:
`public/assets/builtin/solu/{spritesheet.webp,pet.json}` + manifest 追加 `"solu"`。

Run: `cd tools/pet-pipeline && .venv/bin/python -m petpipe pack input/solu --out ../../public/assets/builtin --dry-run`
Expected: 打印 atlas/stateMap/oneShot 概要,animations 含 idle 与已产状态

- [ ] **Step 4: 桌面 QA + Commit**

跑手册 §7 清单,全过后:

```bash
git add public/assets/builtin/solu public/assets/builtin/pets-manifest.json
git commit -m "feat(assets): 官方宠物 小煦(solu)"
```

- [ ] **Step 5: 后续三宠**

Muru/Riffi/Luma 重复 Step 3-4,各自独立 commit。全部完成后
`gh pr create` 出 PR(base: main),PR 描述关联设计文档 §7。

---

## Self-Review 记录

- **Spec 覆盖**:风格圣经 ✓(Task 5)/ prompt 模板 ✓(Task 5)/ 抠图 ✓(Task 4)/ 对齐打包脚本 ✓(Task 1-3)/ pet.json 与 manifest ✓(Task 2、6)/ 桌面 QA ✓(Task 6)/ LoRA 可选项 ✓(Task 5 附录)。版权皮肤下架在并行会话(task_58cadb96),本计划仅做协调约定(Global Constraints + Task 6 rebase 步骤)。
- **占位符扫描**:无 TBD;生成/策展是固有人工环节,已明确为"人机协作步骤"而非代码占位。
- **类型一致性**:`build_pet` 返回 `(Image, dict)` 在 Task 2/3 一致;`STATE_SPECS` 键序与 Task 5 delta 表、Task 2 测试断言一致(9 状态);`make_input` 在 test_packer 定义、test_cli 复用。
