# PawBae 皮肤规范（v1）

给 PawBae 做一个皮肤 = 一个文件夹、一张精灵图集、一个 `pet.json`。在应用里打开 **皮肤工坊 🎨 → 导入文件夹** 即可安装；同 id 重新导入会原地覆盖升级。

**最低门槛：一张图。** 工坊里选 **导入图片**，任意 PNG/WebP/JPG/GIF 都会自动包装成一个站桩皮肤（整张图 = 1 帧 idle），其余动画自动优雅回退。想让她动起来，再照本规范升级成图集即可。

## 文件夹结构

```
my-skin/
├── pet.json          # 清单（必需）
└── spritesheet.webp  # 精灵图集（必需，png/webp/jpg 均可）
```

## 图集格式

- 单张图片按**等宽等高的网格**切分：`cols` 列 × `rows` 行，每格 `cellW` × `cellH` 像素。
- **一行 = 一个动画**，从左往右播放 `frames` 帧。
- 推荐用透明背景 WebP；像素风皮肤把 `imageRendering` 设为 `"pixelated"`。
- 图片尺寸建议不超过 4096×4096。

## 标准 9 行布局（推荐格式）

不声明 `animations` 字段时自动套用此布局，此时格子固定 192×208、8 列 9 行（整图 1536×1872）：

| 行 | 名称 | 帧数 | 默认 fps | 用途 |
|---|---|---|---|---|
| 0 | `idle` | 6 | 2 | 站桩（必备） |
| 1 | `run-right` | 8 | 8 | 向右跑（漫步/回家） |
| 2 | `run-left` | 8 | 8 | 向左跑（漫步/出发） |
| 3 | `waving` | 4 | 12 | 打招呼 |
| 4 | `jumping` | 5 | 6 | 跳跃（单次播放） |
| 5 | `failed` | 8 | 12 | 任务失败垂头 |
| 6 | `waiting` | 6 | 6 | 等你审批（循环间歇 600ms） |
| 7 | `running` | 6 | 6 | agent 干活中 |
| 8 | `review` | 6 | 12 | 检查工作 |

## pet.json 字段参考

```jsonc
{
  "id": "my-skin",            // 必需。安装文件夹名，安全字符（支持中文；不允许 / \ : ..，不以 . 开头，≤64 字符）
  "displayName": "小云",       // 显示名（用户还能再起昵称）
  "description": "一句话介绍",
  "spritesheetPath": "spritesheet.webp",  // 默认 spritesheet.webp
  "imageRendering": "auto",   // auto | pixelated | crisp-edges
  "atlas": { "cellW": 192, "cellH": 208, "cols": 8, "rows": 9 },
  "animations": {             // 省略整个字段 = 套用标准 9 行
    "idle": { "row": 0, "frames": 6, "fps": 2 }
    // row 必需；frames 默认 1；可选 fps、loopRestMs（循环间歇毫秒）、
    // flipX（水平翻转）、offsetCol（起始列）、displayScale（该行缩放）
  },
  "stateMap": {               // agent 状态 → 动画行（默认如下）
    "idle": "idle", "working": "running", "compacting": "running", "waiting": "waiting"
  },
  "oneShot": ["jumping"],     // 播一遍就停的行（默认 ["jumping"]）
  "physics": { "enabled": true }  // 声明后可用攀爬/抛掷物理行
}
```

## 进阶动画词汇表（选做，逐行解锁能力）

引擎按名字找行，缺哪行就回退，**永远不会报错**：

- **物理**（拖拽/抛掷/攀爬）：`falling`、`bouncing`、`grab-wall`、`grab-wall-flipped`、`climb-wall`、`climb-wall-flipped`、`climb-ceiling`、`climb-ceiling-flipped`
- **开饭**：`eat`（缺则用 `happy`，再缺则不播）
- **键鼠反应**：`react-keyboard`、`react-mouse`
- **闲置小动作**（声明即进随机池）：`blink`、`happy`、`thinking`、`pounce`、`yawn`、`sleep`、`rest`、`dance`、`spin`、`peek`
- **语音情绪**：`happy`、`sleep`、`eat`、`angry`
- **任务结果**（单次）：`done-success`、`done-fail`

满配参考：内置 Yoonie（`public/assets/builtin/yoonie/`，31 行）。

## 导入校验（宽进严出）

**拦截导入**：pet.json 非法 JSON、id/图集路径含穿越字符、图集参数非正整数、图集超出图片实际尺寸、声明动画却没有 `idle`、行号/帧数越界。

**只提醒不拦截**：图片未被整除、缺标准行、动画名疑似拼写错误、fps > 60、超大图。

## 分发

v1 皮肤以文件夹/压缩包自由分发（解压后导入文件夹）。皮肤目录在工坊里点「打开皮肤文件夹」可直达（`~/.codex/pets`，与 codex CLI hatch-pet 布局兼容）。
