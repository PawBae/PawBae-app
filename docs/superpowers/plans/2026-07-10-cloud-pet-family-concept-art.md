# PawBae Cloud Pet Family Concept Art Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate and preserve one polished 3D concept board for PawBae’s four approved original cloud-world pets: Solu, Muru, Riffi, and Luma.

**Architecture:** Treat the approved character specification as the source of truth, use the built-in image generation tool for a preview-first concept board, then perform one focused visual QA pass. Preserve only the approved final image in the repository’s documentation assets; do not wire it into the application or treat it as production sprite artwork.

**Tech Stack:** Built-in `image_gen`, local image inspection, PNG artifact, Markdown prompt record

## Global Constraints

- The four characters are original fantasy creatures from Yoonie’s cloud world; none is a recognizable real-world animal.
- Personality is primary; characters do not map to fixed Agent states or jobs.
- Preserve the reference image’s soft fur, candy palette, polished 3D collectible feel, and friendly expression without copying any specific character, costume, facial proportions, silhouette, or name.
- Use a shared heart-shaped cloud core as the only mandatory family mark.
- Keep each character distinguishable by silhouette without relying on text or color.
- The first round is a concept board only: no spritesheet, animation frames, transparent production asset, adoption UI, stats, clothing, rarity, or Yoonie redesign.
- Do not overwrite an existing concept file; if `cloud-pet-family-v1.png` exists, use the next versioned filename.

---

### Task 1: Generate the Four-Character Concept Board

**Files:**
- Read: `docs/superpowers/specs/2026-07-10-cloud-pet-family-design.md`
- Create after approval: `docs/concepts/cloud-pet-family-v1.png`

**Interfaces:**
- Consumes: the approved character names, silhouettes, palettes, personality actions, and avoidance rules from the design specification
- Produces: one preview image containing all four full-body character concepts

- [ ] **Step 1: Verify the generation inputs**

Confirm that the conversation contains the user’s visual reference and that the approved specification contains exactly four characters in this order: Solu, Muru, Riffi, Luma. Treat the user image only as a mood and finish reference, not an edit target.

Expected: one mood reference, one approved specification, and no missing character details.

- [ ] **Step 2: Generate the first concept board with the built-in image tool**

Use this exact normalized prompt:

```text
Use case: stylized-concept
Asset type: PawBae original character-family concept board
Primary request: Create one polished horizontal concept board showing four original fantasy cloud creatures from the same world, arranged left to right as Solu, Muru, Riffi, and Luma. Their appeal comes from distinct personalities rather than jobs or game classes.
Input image role: The user-provided pet-app artwork is a mood and finish reference only. Borrow only the inviting softness, candy-colored 3D collectible polish, and friendly presentation. Do not copy its animals, costumes, facial proportions, silhouettes, UI, or names.
Scene/backdrop: Four clean adjacent pastel studio backdrop panels with subtle gradients, no environment and no interface.
Subject 1 — Solu / 小煦: an original sunny fluff creature, optimistic and outgoing; cream-yellow and peach-orange short fur with a touch of coral; a rounded sunflower-like ear crown that is clearly organic fur rather than petals, round cheeks, tiny limbs, and a fluffy comma-shaped tail; a naturally formed pale heart-shaped patch in the chest fur; leaning forward as if eager to share good news. It must not resemble a lion, fox, flower mascot, or recognizable real animal.
Subject 2 — Muru / 雾露: an original mist-cluster creature, quiet and shy; mist blue, pale lavender, and pearl white; low soft cloud-like body, long drooping cloud ears whose tips are subtly translucent, and a tail that softly dissolves at the edge; a mist-white heart marking naturally formed on the forehead; ears partly hiding the face while curious eyes peek out. It must not resemble a rabbit or ghost.
Subject 3 — Riffi / 雷栗: an original thunder-sprout creature, boisterous and warm but secretly timid; mint-green and soft electric-yellow fur with deep teal accents; rounded lightning-shaped ear tips, bean-like body, tiny round limbs, and a short zigzag tail with a heart-shaped negative-space notch; stepping forward with excited but slightly nervous energy and a tiny halo of static fluff at the ears. It must not resemble a dragon, horned monster, or any familiar electric-animal character.
Subject 4 — Luma / 星沫: an original star-dew creature, dreamy, lazy, and generous; berry-pink and night-sky blue fur with restrained soft-gold accents; crescent-shaped side ears, a relaxed slightly reclined soft body, and a long comet-shaped tail with sparse stardust only at the tip; a soft-gold heart marking near the base of the tail; gently yawning while the tail cradles one tiny dewdrop-like light. It must not resemble a unicorn, star cat, or moon rabbit.
Style/medium: premium stylized 3D character design render, tactile short plush fur, soft rounded construction, collectible-toy charm without plastic shine, refined but not photorealistic.
Composition/framing: wide landscape board, four equal columns, each character shown full body in a front or slight three-quarter view, consistent scale, generous spacing, feet visible, silhouettes never overlap.
Lighting/mood: soft diffused studio lighting, gentle ambient occlusion, warm and sincere, slightly clumsy and lovable.
Color palette: harmonious low-saturation candy colors; each character retains its specified palette and remains distinct from the others.
Text (verbatim): "Solu  小煦"  "Muru  雾露"  "Riffi  雷栗"  "Luma  星沫"
Constraints: large heads, short limbs, low centers of gravity, expressive eyes that are lively but not oversized; one major silhouette idea per character; the heart-shaped cloud core must feel grown into the body rather than attached as a badge; all four must look related without sharing the same recolored body.
Avoid: recognizable real animal species, copied character designs, clothing, accessories, weapons, jobs, game classes, UI cards, buttons, stats, phone mockups, extra characters, complex scenery, hyper-saturated neon, wet skin, hard plastic, adult expressions, exaggerated fashion poses, logos, watermark, and any text beyond the four exact name labels.
```

Expected: one image-generation result with four complete, non-overlapping characters and no app UI.

- [ ] **Step 3: Inspect the first result at original detail**

Check the generated result against this table:

| Check | Pass condition |
| --- | --- |
| Count | Exactly four characters; no extra mascot or background creature |
| Order | Solu, Muru, Riffi, Luma from left to right |
| Silhouette | Ear and tail shapes remain distinguishable in grayscale |
| Personality | Optimistic, shy, boisterous-timid, and dreamy read without labels |
| Family system | All four share soft construction and a natural heart cloud core |
| Originality | No character reads as a direct cat, rabbit, fox, dragon, unicorn, or mouse |
| Presentation | Full bodies and feet are visible; no overlap; no UI or stats |
| Text | Only the four approved names appear; labels are legible enough for concept review |

Expected: either all checks pass or one concise list identifies the highest-impact defects.

---

### Task 2: Perform One Focused Revision and Preserve the Final

**Files:**
- Create: `docs/concepts/cloud-pet-family-v1.png`
- Create: `docs/concepts/cloud-pet-family-v1-prompt.md`

**Interfaces:**
- Consumes: the first generated concept board and the Task 1 acceptance findings
- Produces: one reviewed PNG and a reproducible prompt record

- [ ] **Step 1: Decide whether a revision is necessary**

Revise only when one or more Task 1 checks fail materially. Select the single highest-impact correction category in this priority order: missing or extra subject, copied/recognizable species, broken silhouette, wrong personality, cropped body, UI contamination, then text errors.

Expected: either retain the first result or define one targeted revision instruction; do not broaden the approved design.

- [ ] **Step 2: Generate at most one targeted revision**

Use the first result as the edit target when possible. Preserve every passing element and append exactly one of these correction blocks, choosing the first block that matches the highest-priority failure:

```text
Targeted revision — subject count: Correct only the subject count. Show exactly four full-body characters in this left-to-right order: Solu, Muru, Riffi, Luma. Remove every extra creature or duplicate. Preserve every already-correct silhouette, approved palette, personality cue, natural heart cloud core, panel, material, and lighting choice. Do not add clothing, props, UI, scenery, logos, watermarks, or explanatory text.

Targeted revision — species originality: Correct only the recognizable-animal resemblance. Make the affected character an unmistakably original cloud-world fantasy creature by strengthening its approved ear, body, and tail geometry. Preserve its palette, personality, natural heart cloud core, pose, panel, material, and all three unaffected characters. Do not introduce traits of cats, rabbits, foxes, dragons, unicorns, mice, or familiar electric-animal characters.

Targeted revision — silhouette: Correct only the weak silhouette. Separate all four figures and strengthen the affected character’s approved ear and tail shape so it remains distinct in grayscale. Preserve the order, palettes, faces, personalities, natural heart cloud cores, material, lighting, and every already-correct character.

Targeted revision — personality: Correct only the affected personality cue. Make Solu read optimistic and eager, Muru shy and curious, Riffi boisterous but slightly nervous, and Luma dreamy and relaxed through the approved poses and expressions. Preserve all silhouettes, palettes, heart cloud cores, panels, material, lighting, and already-correct characters.

Targeted revision — framing: Correct only the framing. Show all four characters completely from ear tips to feet and tail tips, at consistent scale, with generous spacing and no overlap. Preserve every design, palette, personality, natural heart cloud core, material, panel, and lighting choice.

Targeted revision — UI contamination: Remove only UI cards, buttons, stats, phone frames, badges, logos, watermarks, and explanatory copy. Preserve the four characters, their order, name labels, silhouettes, palettes, personalities, natural heart cloud cores, panels, material, and lighting.

Targeted revision — labels: Correct only the four labels so the image contains exactly these names in order: “Solu  小煦”, “Muru  雾露”, “Riffi  雷栗”, “Luma  星沫”. Preserve every visual element unchanged and add no other text.
```

Expected: the revision corrects the named defect without redesigning unrelated characters.

- [ ] **Step 3: Run final visual QA**

Re-run all eight acceptance checks from Task 1 on the selected final image. If a name label contains a minor typography defect but the character design is strong, keep the image and record the text limitation rather than spending additional generations; names can be typeset separately in a later layout pass.

Expected: all character-design checks pass; any remaining limitation is confined to generated label typography and is explicitly reported.

- [ ] **Step 4: Copy the selected result into the repository**

Copy the final generated PNG from its built-in image-generation location to:

```text
docs/concepts/cloud-pet-family-v1.png
```

If that path already exists, use `docs/concepts/cloud-pet-family-v2.png` and apply the same version number to the prompt record.

Expected: the final concept image exists inside the isolated worktree and is not left only under the default generated-images directory.

- [ ] **Step 5: Record the final prompt and QA result**

Create `docs/concepts/cloud-pet-family-v1-prompt.md`. Copy the complete Task 1 normalized prompt under `## Final Prompt`. Under `## Targeted Revision`, copy the one correction block used in Step 2, or write `None.` when the first result was selected. Use one of the following two exact metadata headers:

```markdown
# Cloud Pet Family v1 — Generation Record

- Tool path: built-in image generation
- Use case: stylized-concept
- Source specification: `docs/superpowers/specs/2026-07-10-cloud-pet-family-design.md`
- Mood reference role: softness, candy-colored 3D finish, friendly presentation only
- Revision count: 0
- Final QA: pass
```

When one revision was used, replace the final two lines with:

```markdown
- Revision count: 1
- Final QA: pass
```

If the only residual issue is generated label typography, use `- Final QA: pass with label-typography limitation` instead of `- Final QA: pass`.

Expected: the record matches the selected image version and contains no placeholder text.

- [ ] **Step 6: Verify repository scope**

Run:

```bash
git status --short
git diff --check
```

Expected: only the final PNG, its prompt record, and this plan are uncommitted; `git diff --check` exits successfully.

- [ ] **Step 7: Commit the reviewed concept artifact**

```bash
git add docs/concepts/cloud-pet-family-v1.png \
  docs/concepts/cloud-pet-family-v1-prompt.md \
  docs/superpowers/plans/2026-07-10-cloud-pet-family-concept-art.md
git commit -m "art: add cloud pet family concept board"
```

Expected: one commit containing the final concept board, its reproducibility record, and this execution plan.
