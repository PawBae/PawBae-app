# PawBae Four-Pet Onboarding UI — Design Specification

**Date:** 2026-07-10
**Surface:** Screen 1 only — first-run onboarding
**Target:** Svelte 5 + Tauri 2 desktop app
**Approved scope:** High-fidelity UI and real local interactions; GitHub OAuth is an honest optional step with an integration seam, not a simulated login.

## 1. Goal

Replace the current single-modal mode picker with a four-step desktop onboarding experience that makes AI coding agents feel approachable and gets the user to the first value moment: PawBae returning to the transparent desktop stage with the user’s onboarding choices saved.

The flow must feel cozy and playful without weakening trust. It serves developers first, uses familiar desktop controls, explains local processing clearly, supports Chinese and English, and preserves PawBae’s existing privacy defaults.

## 2. Scope

### Included

1. Welcome step.
2. Optional GitHub sign-in step with a future OAuth interface and an honest unavailable state in the current build.
3. Coding-agent connection step for Claude Code, Codex, and Cursor using the existing local Hook commands and settings.
4. Starter-pet adoption step featuring Solu, Muru, Riffi, and Luma.
5. Light and dark theme tokens for this onboarding surface.
6. Keyboard navigation, focus management, reduced motion, loading, success, and error states.
7. Bilingual Simplified Chinese and English UI copy through the existing i18n system.
8. Preservation of the current opt-in telemetry choice, unchecked by default.
9. Completion wiring back into `Main.svelte`: app mode, selected integrations, telemetry consent, selected starter pet, and window restoration.

### Not included

- GitHub OAuth backend, Supabase session persistence, profile creation, or cloud account state.
- Settings panel redesign, desktop-stage redesign, Friends, visiting, or Memory Card screens.
- New pet spritesheets or animation production.
- Changes to Agent monitoring behavior beyond invoking existing Hook installation commands and persisting the selected toggles.
- Replacing or removing the existing skin workshop.

## 3. Current-State Findings

- `Onboarding.svelte` currently presents two mode cards in a dark 420px modal and completes immediately after one click.
- `Main.svelte` opens onboarding when `settingsStore.appMode` is unset and calls `set_mini_size({ restore: false })` before rendering it.
- `settingsStore` already persists `appMode`, telemetry consent, the three integration toggles, and `miniPetId`.
- Hook install commands exist for local agent integrations. The onboarding must call the real command and show the real result instead of faking connection.
- GitHub OAuth exists only in platform planning documents. No auth client or persisted session API is currently callable from the onboarding.
- Solu, Muru, Riffi, and Luma are not yet bundled Codex pet sprites. The approved four-pet poster is therefore a presentation asset for adoption; the chosen starter id is persisted for forward compatibility, while the desktop stage falls back to Yoonie until that pet’s production sprite exists. Producing those stage sprites belongs to Screen 2 rather than this Screen 1 implementation.

## 4. Information Architecture

The onboarding is one `960 × 600` maximum-size desktop surface inside the current Tauri window. On smaller displays it fits within the existing 85%-of-monitor settings frame and never scales typography fluidly.

The top region contains:

- PawBae identity on the left;
- a four-step progress indicator in the center;
- `Set up later / 稍后设置` on the right.

The four steps are:

1. `welcome`
2. `github`
3. `agents`
4. `adopt`

Only one step is visible at a time. Back and Continue remain in a fixed footer so the window does not jump between steps.

### Completion semantics

- Selecting one or more integrations completes onboarding in `coding` mode.
- Selecting no integrations and choosing `Just keep me company / 先让它陪陪我` completes in `pet` mode.
- `Set up later` also completes in `pet` mode, keeps Yoonie as the current stage pet, and preserves telemetry as disabled.
- The adoption step requires an explicit pet selection before its primary CTA is enabled.
- Completing adoption persists the selected official starter id. If no renderable skin exists for that id, `skinsStore.resolve()` continues to render Yoonie without an error dialog.

## 5. Step Specifications

### Step 1 — Welcome

**Heading:** `Meet the gentler side of AI agents / 让 AI Agent 更亲近一点`

The left side contains the value proposition and two concise trust statements. The right side uses the approved four-pet poster as the dominant brand image.

Required messages:

- `Your pet reacts when Claude Code, Codex, or Cursor works. / 当 Claude Code、Codex 或 Cursor 工作时，宠物会回应你。`
- `Agent activity is processed locally. / Agent 活动只在本机处理。`
- Honest time estimate: `About 1 minute / 大约 1 分钟`.

Telemetry remains a separate unchecked checkbox in this step. The copy must explain anonymous product analytics without bundling consent into Continue.

### Step 2 — GitHub

**Heading:** `Connect your identity—not your code / 连接身份，不读取你的代码`

The surface reserves the final GitHub sign-in layout but receives capability through an optional callback interface:

```ts
export interface GithubProfile {
  login: string;
  displayName?: string;
  avatarUrl?: string;
}

onGithubSignIn?: () => Promise<GithubProfile>;
```

When the callback is absent, the current build shows:

- disabled `Continue with GitHub / 使用 GitHub 登录` button;
- status text `GitHub sign-in opens with the Friends beta. / GitHub 登录将在好友内测开放。`;
- active secondary action `Skip for now / 暂不登录`.

The step must never show a fabricated avatar, handle, success check, or authenticated state.

### Step 3 — Coding Agents

**Heading:** `Which agent should your pet listen to? / 让宠物听见哪个 Agent？`

Claude Code, Codex, and Cursor are compact selectable rows, not large marketing cards. Each row includes:

- product name and neutral monochrome icon;
- one-line explanation;
- checkbox or switch;
- platform availability;
- local installation status: idle, installing, connected, or failed.

Selecting an agent installs its Hook immediately. A failure keeps the row selected only if the Hook is actually active; otherwise it reverts and exposes a concise retry action.

Windows availability follows current product behavior: unsupported integrations remain visible but disabled with `Not available on Windows yet / Windows 暂不可用`, so the list does not appear to have missing products.

The footer includes `Just keep me company / 先让它陪陪我`, which advances with no integrations selected.

### Step 4 — Adopt

**Heading:** `Choose your first desktop companion / 选择第一位桌面伙伴`

Four equal adoption cards remain in one horizontal row throughout the supported desktop range, including `760 × 600`. Below 820px, the cards become denser by reducing gaps, artwork height, and internal padding; they do not wrap or introduce horizontal scrolling. Each card includes a poster crop, bilingual name, and one personality line:

- `Solu / 小煦 — Warm & sunny / 温暖又开朗`
- `Muru / 雾露 — Shy & soothing / 安静又治愈`
- `Riffi / 雷栗 — Energetic & clumsy / 热情但有点笨拙`
- `Luma / 星沫 — Sleepy & dreamy / 慵懒的幻想家`

Cards begin unselected. Selection is conveyed by all of:

- 2px pet-theme outline;
- checkmark icon;
- selected label announced to assistive technology;
- pet name inserted into the final CTA.

The CTA follows `Adopt Solu / 领养小煦` and changes with selection. Notes state `You can change pets anytime / 以后可以随时更换` and, for this milestone, `Your choice will be saved; animated desktop forms arrive with the Stage update. / 你的选择会被保存；动态桌面形态将在舞台更新中加入。` This avoids implying that poster art is already a working sprite.

## 6. Visual System

### Neutral tokens

| Token | Light | Dark |
| --- | --- | --- |
| canvas | `#FBFAF8` | `#242226` |
| surface | `#FFFFFF` | `#2D2A2F` |
| subtle surface | `#F4F1F1` | `#36323A` |
| primary text | `#2E2B31` | `#F7F3F5` |
| secondary text | `#635E67` | `#C8C0C8` |
| border | `#DED8DC` | `#514B55` |
| primary action | `#3D4E9E` | `#B3C7F0` |
| action text | `#FFFFFF` | `#242226` |
| focus ring | `#596BC0` | `#C9D6F5` |

The top-right appearance control offers `System / Light / Dark` (`跟随系统 / 白色 / 黑色`). `System` resolves through `prefers-color-scheme`; the two explicit choices override it. The preference is stored locally under `pawbae-onboarding-theme` and restored when onboarding opens again. Theme changes affect the neutral surfaces and controls only; pet artwork and pet identity colors remain stable.

### Pet tokens

- Solu: `#FFD98E`, `#FFB36B`, `#F58F5E`, strong `#9C472F`.
- Muru: `#C9D6F5`, `#B3C7F0`, `#E6E9FA`, strong `#455A96`.
- Riffi: `#BFE8D2`, `#A8E0C0`, `#F5E39A`, strong `#2E6C58`.
- Luma: `#F5AFC8`, `#3D4E9E`, `#E8C86A`, strong `#7E4160`.

Pet colors occupy no more than 14% of generic tool surfaces. They may fill the adoption artwork and selected card tint.

### Typography

- Rounded headings: `M PLUS Rounded 1c`, `Noto Sans SC`, system sans.
- Body and controls: `Inter`, `Noto Sans SC`, `Segoe UI`, `PingFang SC`, sans-serif.
- Window title: 24/32, 650.
- Section title: 20/28, 650.
- Card title: 16/24, 650.
- Body: 14/21, 400.
- Compact UI: 13/18, 450.
- Caption: 12/16, 450.

### Shape and elevation

- Inputs and buttons: 10px radius.
- Standard panels and bubbles: 14px.
- Adoption cards: 16px.
- Window shell: 18px.
- Tags only: full pill.
- Normal cards use either a border or background contrast, not a border plus a wide decorative shadow.
- Floating window content may use `0 4px 8px rgba(38, 30, 36, 0.14)`.
- Felt texture is limited to the poster/art region at 3–5% opacity and never appears behind body copy.

## 7. Component Boundaries

`Onboarding.svelte` owns the flow state and composition but delegates repeated units:

- `OnboardingProgress.svelte`: four-step progress and accessible current-step announcement.
- `AgentConnectionRow.svelte`: one integration’s selection, install status, retry, and platform availability.
- `PetAdoptionCard.svelte`: one pet’s artwork crop, name, personality, selection, and theme tokens.

Pure onboarding types and transition rules live in `src/lib/utils/onboarding.ts` so they can be tested without rendering Svelte:

```ts
export type OnboardingStep = 'welcome' | 'github' | 'agents' | 'adopt';
export type OfficialPetId = 'solu' | 'muru' | 'riffi' | 'luma';
export type AgentId = 'claude' | 'codex' | 'cursor';

export interface OnboardingResult {
  mode: AppMode;
  shareTelemetry: boolean;
  selectedAgents: AgentId[];
  starterPetId: OfficialPetId | null;
  githubProfile: GithubProfile | null;
}
```

`Main.svelte` receives one `OnboardingResult`, persists local settings, resolves the renderable pet, restores the mini window, and starts polling through the existing app-mode effect.

## 8. Asset Strategy

The approved generated poster is copied into the project as:

`public/assets/onboarding/pet-family-poster.png`

The adoption cards reuse the master image with four CSS crop positions. This keeps the first implementation faithful to the approved poster without creating fake sprites or four independently drifting assets. The full poster remains visible on the Welcome step.

The poster is presentation-only and must not be registered in `pets-manifest.json` as a working desktop pet.

## 9. Error and Edge States

- Hook install in progress: disable that row only and show a compact progress label.
- Hook install failure: show the returned error in a two-line maximum region with Retry; do not block other integrations.
- Settings persistence failure on final completion: keep onboarding open and show one footer error; do not partially dismiss the flow.
- Poster load failure: preserve names and personality copy in stable card dimensions with a neutral cloud placeholder.
- Missing GitHub callback: show the explicit Friends-beta unavailable state.
- Unsupported platform: disabled integration row with reason.
- Reopening onboarding is out of scope; Settings remains the post-onboarding change surface.

## 10. Accessibility and Interaction

- Focus enters the visible step heading when the step changes.
- Tab order follows header, content, secondary action, primary action.
- Left and right arrow keys move between adoption cards; Enter selects and confirms only after selection.
- Every selectable card uses a real button or radio-group pattern, never a click-only `div`.
- Body text and placeholders target at least 4.5:1 contrast; large text targets 3:1.
- Visible 2px focus rings are never removed.
- Status is conveyed by text/icon plus color.
- Standard transitions use 160–220ms ease-out. `prefers-reduced-motion` removes positional motion and retains immediate state feedback.
- Chinese and English strings may expand by 30% without clipping; English and Chinese are not concatenated with slashes inside buttons.

## 11. Verification

### Unit tests

- Step ordering and back/next boundaries.
- App-mode derivation from selected integrations.
- Explicit pet-selection requirement.
- Skip semantics and telemetry default-off behavior.
- Official pet metadata and CTA copy lookup.

### Component tests

- GitHub unavailable state never reports authentication.
- Agent rows expose loading, success, failure, retry, and unsupported states.
- Adoption cards provide keyboard and screen-reader selection semantics.
- Completion emits the exact `OnboardingResult`.

### Visual verification

- Light and dark screenshots at `960 × 600`.
- Narrow fallback at `760 × 600` keeps all four adoption cards in one row without clipped text or horizontal scrolling.
- System, Light, and Dark appearance choices update immediately and restore from local storage.
- macOS and Windows window framing.
- Chinese and English at 100% and Windows UI-scale zoom.
- Reduced-motion behavior.

## 12. Acceptance Criteria

- A first-run user can finish in approximately one minute.
- No unavailable cloud capability is presented as working.
- At least one real local agent Hook can be installed and its actual result shown.
- The four official pets are the emotional focus of adoption without making the rest of the UI resemble a toy store.
- Completing or skipping produces a valid existing PawBae app mode and restores the desktop stage; until official stage sprites are produced, the stage honestly uses its existing Yoonie fallback.
- Telemetry remains an explicit unchecked opt-in.
- The screen works in light and dark themes, Chinese and English, mouse and keyboard.
