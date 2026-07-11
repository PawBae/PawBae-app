// Canvas renderer for the weekly share card. Deliberately dumb: every string is
// pre-composed (i18n lives in the caller), every number pre-formatted by
// utils/weekly-report.ts — this file only knows positions, fonts, and paint.
// 1080×1440 (3:4) suits both 小红书 and X timelines.

export const CARD_W = 1080;
export const CARD_H = 1440;

const BG_TOP = '#1a1a20';
const BG_BOTTOM = '#24242c';
const ACCENT = '#6495ED';
const FONT = '-apple-system, "PingFang SC", "Segoe UI", "Noto Sans", sans-serif';

export interface ShareCardContent {
  weekLabel: string;
  /** e.g. "⭐ Junior" — stage emoji + localized stage name. */
  stageLine: string;
  heroLabel: string;
  /** Pre-formatted hero number, e.g. "2.4M" / "240万". */
  heroNumber: string;
  /** Unit after the hero number, e.g. "tokens". */
  heroSuffix: string;
  /** e.g. "🤖 38 agent tasks · 💬 512 messages". */
  statsLine: string;
  /** e.g. "🔥 12-day streak 🛡️🛡️" — empty string skips the row. */
  streakLine: string;
  /** e.g. "和 Yoonie 相伴第 45 天". */
  togetherLine: string;
  dailyTokens: number[];
}

export interface SpriteFrame {
  image: CanvasImageSource;
  sx: number;
  sy: number;
  sw: number;
  sh: number;
}

function centerText(
  ctx: CanvasRenderingContext2D,
  text: string,
  y: number,
  font: string,
  fill: string,
) {
  ctx.font = font;
  ctx.fillStyle = fill;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'alphabetic';
  ctx.fillText(text, CARD_W / 2, y);
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
  ctx.fill();
}

/** Renders the whole card. `sprite: null` falls back to a big 🐾. */
export function renderShareCard(
  canvas: HTMLCanvasElement,
  content: ShareCardContent,
  sprite: SpriteFrame | null,
): void {
  canvas.width = CARD_W;
  canvas.height = CARD_H;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const bg = ctx.createLinearGradient(0, 0, 0, CARD_H);
  bg.addColorStop(0, BG_TOP);
  bg.addColorStop(1, BG_BOTTOM);
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, CARD_W, CARD_H);

  // Header: brand left, week range right.
  ctx.textBaseline = 'alphabetic';
  ctx.font = `700 52px ${FONT}`;
  ctx.fillStyle = 'rgba(255,255,255,0.92)';
  ctx.textAlign = 'left';
  ctx.fillText('🐾 PawBae', 72, 108);
  ctx.font = `500 40px ${FONT}`;
  ctx.fillStyle = 'rgba(255,255,255,0.5)';
  ctx.textAlign = 'right';
  ctx.fillText(content.weekLabel, CARD_W - 72, 108);

  // Pet portrait, centered in a 420px box. Pixel art stays crisp.
  const box = { x: (CARD_W - 420) / 2, y: 170, size: 420 };
  if (sprite) {
    const scale = Math.min(box.size / sprite.sw, box.size / sprite.sh);
    const dw = sprite.sw * scale;
    const dh = sprite.sh * scale;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(
      sprite.image,
      sprite.sx,
      sprite.sy,
      sprite.sw,
      sprite.sh,
      box.x + (box.size - dw) / 2,
      box.y + (box.size - dh) / 2,
      dw,
      dh,
    );
    ctx.imageSmoothingEnabled = true;
  } else {
    centerText(ctx, '🐾', box.y + 300, `280px ${FONT}`, 'rgba(255,255,255,0.9)');
  }

  centerText(ctx, content.stageLine, 680, `600 44px ${FONT}`, 'rgba(255,255,255,0.85)');

  // Hero number with its label above and unit beside.
  centerText(ctx, content.heroLabel, 790, `500 40px ${FONT}`, 'rgba(255,255,255,0.55)');
  ctx.font = `800 150px ${FONT}`;
  const numW = ctx.measureText(content.heroNumber).width;
  ctx.font = `600 44px ${FONT}`;
  const sufW = content.heroSuffix ? ctx.measureText(content.heroSuffix).width + 20 : 0;
  const startX = (CARD_W - numW - sufW) / 2;
  ctx.textAlign = 'left';
  ctx.font = `800 150px ${FONT}`;
  ctx.fillStyle = '#ffffff';
  ctx.fillText(content.heroNumber, startX, 920);
  if (content.heroSuffix) {
    ctx.font = `600 44px ${FONT}`;
    ctx.fillStyle = 'rgba(255,255,255,0.55)';
    ctx.fillText(content.heroSuffix, startX + numW + 20, 920);
  }

  // 7-day bar chart. Zero days keep a visible base so an empty week still reads
  // as a chart, not a rendering bug.
  const chart = { x: 190, y: 960, w: 700, h: 150 };
  const bars = content.dailyTokens.length || 1;
  const barW = 64;
  const gap = (chart.w - bars * barW) / Math.max(1, bars - 1);
  const max = Math.max(1, ...content.dailyTokens);
  content.dailyTokens.forEach((v, i) => {
    const h = Math.max(10, (Math.max(0, v) / max) * chart.h);
    ctx.fillStyle = i === bars - 1 ? ACCENT : 'rgba(100,149,237,0.45)';
    roundRect(ctx, chart.x + i * (barW + gap), chart.y + chart.h - h, barW, h, 8);
  });

  centerText(ctx, content.statsLine, 1210, `500 40px ${FONT}`, 'rgba(255,255,255,0.75)');
  if (content.streakLine) {
    centerText(ctx, content.streakLine, 1280, `600 40px ${FONT}`, 'rgba(255,180,90,0.9)');
  }

  centerText(ctx, content.togetherLine, 1372, `500 36px ${FONT}`, 'rgba(255,255,255,0.55)');
  centerText(ctx, 'pawbae.ai', 1418, `500 30px ${FONT}`, 'rgba(255,255,255,0.3)');
}
