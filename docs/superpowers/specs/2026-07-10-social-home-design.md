# PawBae Social Home — Design Specification

Date: 2026-07-10
Status: Approved concept, ready for implementation planning
Related: [Onboarding UI](2026-07-10-onboarding-ui-design.md), [Social Visiting](2026-07-09-social-visiting-design.md)

## 1. Product decision

After onboarding, PawBae opens a `960 × 600` Home window before the pet is sent to the transparent desktop stage. Home is not an agent dashboard. It is a **pet social living room** where the user's adopted pet remains the emotional center and friendships are expressed through visits and shared memories.

The primary loop is:

`friend → invite a pet to visit → spend time together → return home → receive one shared memory`

Coding-agent activity remains part of the pet's presence, but it is translated into a short, privacy-safe pet status instead of occupying a large operational card.

## 2. Goals

1. Give adoption a satisfying destination before the pet moves onto the desktop.
2. Make visiting another pet the clearest social action on Home.
3. Let the user understand who is home, away, visiting, or waiting without opening a management screen.
4. Keep the user's own pet visually dominant in the idle state and show both pets with equal dignity during a visit.
5. Carry the onboarding light/dark visual system into the post-onboarding product.
6. Preserve developer trust: agent activity is useful and legible, but no task text, code, prompt, path, or approval content is exposed to friends.
7. Keep Home compact enough to sit beside an editor and provide a direct transition to the transparent desktop pet.

## 3. Non-goals

- Home is not a chat client, analytics dashboard, task manager, or social feed.
- v1 does not include public rooms, stranger matching, multiple simultaneous visitors, relationship leaderboards, visit streaks, or punitive daily tasks.
- The Plaza entry is a future discovery surface. It may appear as a clearly labeled preview or disabled entry, but v1 must not imply that public discovery already works.
- Home does not expose full agent sessions. Detailed local agent information remains in the existing agent/session surfaces.
- Shared memories are not published automatically.

## 4. Window and navigation model

### 4.1 Onboarding completion

Completing the adoption step opens Home at `960 × 600` rather than immediately restoring the `200 × 200` transparent stage. The selected pet, language, theme, and agent connections carry into Home without another setup step.

The primary Home CTA is `Send Muru to desktop / 把雾露放到桌面`, with the pet name resolved from the current selection. Activating it closes or hides Home and restores the transparent always-on-top stage.

Home can later be reopened from the pet, tray menu, or a compact stage action. Reopening Home must not restart onboarding.

### 4.2 Persistent Home regions

Home has four persistent regions:

1. **Identity capsule**, top-left: pet portrait, pet name, life stage when available, privacy-safe companion status, affection, and coins.
2. **Living-room stage**, center: the user's pet, one optional visitor slot, speech bubble, soft ground shadow, and pet-specific ambient light.
3. **Social dock**, right: Friends, Plaza, and Album. Friends and Album are functional v1 destinations; Plaza is visibly marked `Soon / 即将开放` until its product phase exists.
4. **Care and transition bar**, bottom: relationship progress where supported, care actions, diary, and the primary `Send to desktop` action.

Settings and appearance controls remain at the top-right and do not compete with the social dock.

## 5. Default layout

The `960 × 600` shell uses a calm open canvas rather than a grid of dashboard cards.

- Outer window padding: `24px`.
- Identity capsule: approximately `300 × 72`, aligned top-left.
- Social dock: `72px` wide, vertically centered at the right edge.
- Living-room stage: approximately `520 × 360`, centered with a slight left bias so a slide-over can open without hiding the pet entirely.
- Pet visual box: up to `240 × 260` for one pet; two `190 × 220` boxes during a visit.
- Bottom bar: `64–76px` high with compact progress on the left and actions centered/right.
- Right slide-over: `360–384px` wide, inset `16px` from the window edge, with the stage still partially visible.

The idle stage avoids permanent cards around the pet. A single contextual event card may appear below or beside the stage when there is a friend request, visit invitation, return event, or new shared memory.

## 6. Information hierarchy

### Level 1 — presence

- Which pet is here?
- Is it at home, away, or hosting a visitor?
- Is a friend asking to visit?
- Can the pet be sent to the desktop?

### Level 2 — relationship

- Which friends are online or available?
- Who visited recently?
- Is there a new shared memory?
- Can the user invite a specific friend or accept an invitation?

### Level 3 — agent context

- `Working / 正在工作`
- `Waiting / 等待主人回复`
- `Compacting / 正在整理记忆`
- `Idle / 陪着主人`
- `Offline / 主人的 Agent 在休息`

Agent details are revealed only after the user activates the status. Friends receive only the established public state enum and never local task copy.

## 7. Core Home states

### 7.1 At home, no visitor

The user's pet occupies the center stage. A short speech bubble can reference local companion context, such as `Let's take it one step at a time. / 慢慢来，我会陪着你。`

The event area favors one useful prompt:

- a friend is available to invite;
- a pending friend request;
- a visit invitation;
- the most recent shared memory;
- otherwise no card, preserving calm negative space.

### 7.2 Incoming visit request

The event card reads, for example, `Momo wants to bring Solu over. / Momo 想带小煦来玩。` Actions are `Later / 稍后` and `Welcome them / 欢迎它`.

Acceptance is explicit for the first visit. Declining, delaying, or expiry uses neutral copy and has no relationship or resource penalty.

### 7.3 Hosting a visitor

The visitor enters from the edge of the stage in a short travel animation. Both pets remain visually clear:

- the user's pet has no owner tag;
- the visitor has a compact guest tag such as `Momo's partner · Visiting / Momo 的伙伴 · 来访中`;
- both retain their own artwork and identity colors;
- neither pet is visually treated as an inventory item or collectible card.

The bottom actions become `Play together / 一起玩`, `Offer a snack / 请吃点心`, `Take a photo / 拍合照`, and `End visit / 结束串门`.

Hosting actions create positive animation and structured memory material only. They do not change the visitor's hunger, coins, growth, inventory, or ownership.

### 7.4 Pet visiting a friend

Because a pet has one platform location, the user's living-room stage shows an empty nest, footprints, and a status sign such as `Muru is visiting Momo. / 雾露去 Momo 家玩了。` The user can view visit status or recall the pet.

Home must not render a duplicate pet while it is away. Agent and settings entry points remain available without the pet body.

### 7.5 Returning and shared memory

When a visit ends, the visitor says goodbye and leaves. The user's pet returns to its own Home when applicable. One shared memory is settled per visit and announced with restrained feedback:

`Today's shared memory is tucked away. / 今天的共同记忆已经收好。`

The event card offers `View memory / 查看记忆`. The memory uses approved template keys and safe parameters only; it never contains task free text.

### 7.6 Offline and recovery

- If the visitor's owner connector is offline, the visitor sleeps and shows `Their owner's agent is resting. / 主人的 Agent 在休息。`
- If Realtime is degraded, the visit shows last-known safe state and a quiet reconnect indicator.
- A locally known lease expiry removes the visitor or restores the pet home even if an end event was missed.
- Duplicate acceptance, retry, or reconnect must not create two visitors or two memories.

## 8. Social surfaces

### 8.1 Friends slide-over

The Friends panel opens over the right side while leaving the living-room stage partially visible. It contains:

- pending friend requests at the top;
- friends with pet identity, owner handle, current availability, and safe public status;
- one primary row action: `Visit / 去 TA 家玩`, `Invite / 邀请来玩`, `Recall / 召回`, or disabled reason;
- handle search and invitation-link copy at the bottom;
- relationship management in a secondary menu, including mute, unfriend, and block.

Only mutual friends may start a visit. Stranger visits are not allowed by default.

### 8.2 Album slide-over

The Album panel shows two-column Polaroid-style memory cards. Copy states clearly that memories belong to both pets and are not shared publicly by default.

Cards show artwork, a relationship-safe title, date, and participating pets. Opening a card reveals the memory copy and privacy controls. Removing a local display copy must not alter the other participant's copy.

### 8.3 Plaza future entry

The dock may include Plaza for continuity with the product vision, but before public discovery ships it must display `Soon / 即将开放` and must not show fabricated people, activity, or availability.

## 9. Visual system

Home reuses the onboarding neutral tokens:

| Token | Light | Dark |
| --- | --- | --- |
| Canvas | `#FBFAF8` | `#242226` |
| Surface | `#FFFFFF` | `#2D2A2F` |
| Subtle surface | `#F4F1F1` | `#36323A` |
| Primary text | `#2E2B31` | `#F7F3F5` |
| Secondary text | `#635E67` | `#C8C0C8` |
| Border | `#DED8DC` | `#514B55` |
| Primary action | `#3D4E9E` | `#B3C7F0` |
| Focus ring | `#596BC0` | `#C9D6F5` |

The current pet supplies ambient stage light, progress tint, and selected state. Pet color occupies no more than 14% of generic tool surfaces. A visitor keeps its identity color without recoloring the entire Home.

- Window shell: `18px` radius.
- Slide-over and event panels: `20–24px` radius.
- Buttons and inputs: `10–12px` radius.
- Dock actions and tags: full pill where appropriate.
- Floating shadow: no stronger than `0 8px 24px rgba(38, 30, 36, 0.14)`.
- Felt texture: `3–5%` opacity on stage lighting or artwork regions only, never behind body copy.

Typography continues to use rounded CJK-capable headings and high-readability body text. Home uses the onboarding type scale, with no body text below `12px`.

## 10. Motion and sound

- Standard UI transitions: `160–220ms` ease-out.
- Slide-over: approximately `220ms`.
- Visitor arrival/exit: approximately `500ms`, with no full-screen flash or confetti.
- Pet breathing and idle motion remain slow and low amplitude.
- `prefers-reduced-motion` removes translation and keeps immediate opacity/state feedback.
- Visit request and completion sounds honor the global sound preference and never repeat on reconnect.

## 11. Copy and localization

Chinese and English are alternate locales, not simultaneous labels. Pet names resolve from the selected official identity.

| Intent | Chinese | English |
| --- | --- | --- |
| Local agent working | 雾露正在陪主人写代码 | Muru is keeping you company while you code |
| Visitor request | Momo 想带小煦来玩 | Momo wants to bring Solu over |
| Welcome | 欢迎它 | Welcome them |
| Away state | 雾露去 Momo 家玩了 | Muru is visiting Momo |
| Shared memory ready | 今天的共同记忆已经收好 | Today's shared memory is tucked away |
| Desktop transition | 把雾露放到桌面 | Send Muru to desktop |

Never-punish copy rules apply: no blame for declining, expiry, recall, disconnect, or missed visits.

## 12. Privacy and trust

1. Home may show local agent details to the owner, but Friends and visiting views only consume the minimal public pet projection.
2. Public projection is limited to pet identity, appearance, owner display identity, version, and the approved activity enum.
3. Friends cannot read or respond to approvals, run tools, change the owner's pet, access the owner's device, or view task text.
4. Visit invitations must be rate-limited and support mute, per-friend auto-welcome, unfriend, recall, and block.
5. Shared memories use structured facts only and are private to the two pets by default.
6. Plaza remains non-functional until its separate privacy and discovery design is approved.

## 13. Component boundaries

Suggested frontend boundaries:

- `Home.svelte`: window composition and Home-level routing.
- `PetIdentityCapsule.svelte`: identity, safe status, affection, and currency.
- `SocialLivingRoom.svelte`: local pet slot, visitor slot, away state, speech, and stage lighting.
- `SocialDock.svelte`: Friends, Plaza, Album, settings, and active state.
- `HomeEventCard.svelte`: one prioritized request, return, or memory event.
- `VisitActionBar.svelte`: context-aware care, visit, recall, and desktop actions.
- `FriendsPanel.svelte`: friend requests, rows, handle search, and visit actions.
- `SharedAlbumPanel.svelte`: shared memory grid and detail entry.
- `AgentCompanionStatus.svelte`: owner-only detail disclosure and friend-safe summary.

Pure logic should own event priority, visit-state-to-copy mapping, permitted actions, and public/private agent projections. Components should not infer authorization from visual state.

## 14. Accessibility

- All dock, panel, visit, and care actions are keyboard reachable with visible focus.
- Opening a slide-over moves focus to its heading; closing returns focus to the invoking dock action.
- Incoming invitations use a polite live-region announcement but never steal focus.
- Pet artwork has concise identity/state alternatives; decorative light and texture are hidden from assistive technology.
- Color is never the only representation of online, waiting, away, or disabled state.
- Light and dark themes meet WCAG AA for text and interactive controls.
- The Home layout remains usable under Windows UI scaling and at increased text size without hiding the primary visit or desktop action.

## 15. Verification map

### Pure logic

- Home event prioritization selects at most one contextual event card.
- Visit state maps to the correct permitted actions.
- Away state cannot render the local pet body.
- Visitor public projection strips all task and approval text.
- Visit expiry and completion settle at most one shared memory.

### Component behavior

- Onboarding completion opens Home with the chosen pet and theme.
- `Send to desktop` restores the transparent mini stage.
- Friends and Album panels preserve the partially visible stage and return focus on close.
- First visit requires explicit acceptance.
- Hosting shows one visitor maximum and a clear guest tag.
- Plaza does not fabricate a live discovery experience before launch.

### Visual verification

- Capture `960 × 600` light and dark Home states for Solu, Muru, Riffi, and Luma.
- Capture idle, incoming request, hosting, away, memory-ready, offline, and degraded-Realtime states.
- Verify Chinese and English at 100%, 125%, and 150% Windows UI scale.
- Verify reduced motion and keyboard-only operation.

## 16. Acceptance criteria

The design is accepted when:

1. A new user lands in Home after adoption and can send the selected pet to the desktop.
2. The pet remains the strongest visual element; the page does not resemble an agent dashboard.
3. A user can find a mutual friend, invite or visit them, understand availability, and safely decline or recall.
4. Hosting renders exactly two clearly identified pets and exposes only safe activity state.
5. An away pet is not duplicated at home.
6. Completing a visit produces at most one private shared memory with no task free text.
7. Light, dark, Chinese, English, keyboard, reduced-motion, offline, and reconnect states remain usable.
8. Friends and Album are first-class Home destinations, while Plaza is honestly marked as future work.
