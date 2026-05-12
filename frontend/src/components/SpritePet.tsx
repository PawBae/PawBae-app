import { useEffect, useRef, useState } from 'react'
import {
  fpsFor,
  loopRestMsFor,
  animationFor,
  type CodexPet,
  type CodexPetState,
} from '../lib/codexPet'

interface SpritePetProps {
  pet: CodexPet
  state: CodexPetState
  // Visual width in CSS pixels. Height is derived from the pet's
  // declared cell aspect ratio (atlas.cellH / atlas.cellW).
  size: number
  // Fired once a one-shot state reaches its last frame so the parent
  // can flip back to its previous resting state. No-op for looping
  // states.
  onOneShotEnd?: () => void
  // When true, treat one-shot states as looping. Used by hover
  // interactions where the parent wants the animation to keep playing
  // as long as the cursor is over the mascot.
  loop?: boolean
  className?: string
  style?: React.CSSProperties
}

// Render a single atlas cell, advancing frames at the pet's per-state
// fps via requestAnimationFrame. Looping states cycle indefinitely;
// one-shot states (default `jumping`, configurable per-pet via
// `pet.oneShot`) hold the last frame and notify the parent via
// onOneShotEnd. Pets that omit a row gracefully fall back to `idle`
// instead of throwing.
export function SpritePet({ pet, state, size, onOneShotEnd, loop, className, style }: SpritePetProps) {
  const [frameIndex, setFrameIndex] = useState(0)
  const stateRef = useRef(state)
  const loopRef = useRef(loop ?? false)
  const onOneShotEndRef = useRef(onOneShotEnd)
  const oneShotFiredRef = useRef(false)
  const petRef = useRef(pet)
  const clockResetRef = useRef(0)
  // Timestamp until which the current looping cycle is "resting" on the
  // last frame. 0 means no rest in flight.
  const restUntilRef = useRef(0)

  useEffect(() => {
    const prevPet = petRef.current
    const prevState = stateRef.current
    const prevRow = animationFor(prevPet, prevState) ?? animationFor(prevPet, 'idle')
    const nextRow = animationFor(pet, state) ?? animationFor(pet, 'idle')
    const canCarryFrame =
      !!prevRow
      && !!nextRow
      && prevPet.spritesheetUrl === pet.spritesheetUrl
      && prevRow.row === nextRow.row
      && prevRow.frames === nextRow.frames
      && (prevRow.offsetCol ?? 0) === (nextRow.offsetCol ?? 0)

    stateRef.current = state
    oneShotFiredRef.current = false
    restUntilRef.current = 0
    // Same-row transitions (for example run-left/run-right or closely
    // related physics states packed into one row) should keep the frame
    // cadence. Resetting the clock there inserts a tiny visible pause.
    if (!canCarryFrame) clockResetRef.current += 1
    setFrameIndex((prev) => (canCarryFrame && nextRow ? Math.min(prev, nextRow.frames - 1) : 0))
  }, [pet, state])

  useEffect(() => {
    loopRef.current = loop ?? false
  }, [loop])

  useEffect(() => {
    onOneShotEndRef.current = onOneShotEnd
  }, [onOneShotEnd])

  useEffect(() => {
    petRef.current = pet
  }, [pet])

  useEffect(() => {
    let raf = 0
    let acc = 0
    let last = performance.now()
    let clockResetVersion = clockResetRef.current
    let cancelled = false

    const tick = (now: number) => {
      if (cancelled) return
      if (clockResetVersion !== clockResetRef.current) {
        clockResetVersion = clockResetRef.current
        acc = 0
        last = now
      }
      // If we're in an inter-cycle rest, hold the last frame and skip
      // frame advancement until the rest deadline passes.
      if (restUntilRef.current > 0) {
        if (now < restUntilRef.current) {
          last = now
          acc = 0
          raf = requestAnimationFrame(tick)
          return
        }
        // Rest finished: restart the cycle from frame 0.
        restUntilRef.current = 0
        setFrameIndex(0)
        last = now
        acc = 0
        raf = requestAnimationFrame(tick)
        return
      }
      // Re-read the per-frame interval each iteration so per-state fps
      // overrides take effect immediately when the state changes mid-tick.
      const curPet = petRef.current
      const frameMs = 1000 / fpsFor(curPet, stateRef.current)
      const dt = now - last
      last = now
      // WebViews can stall briefly during native-window moves or IPC.
      // Capping catch-up avoids jumping several sprite frames at once after
      // a hiccup, which reads as a broken run/climb cycle.
      acc = Math.min(acc + dt, frameMs * 1.5)
      while (acc >= frameMs) {
        if (restUntilRef.current > 0) break
        acc -= frameMs
        const cur = stateRef.current
        const row = animationFor(curPet, cur) ?? animationFor(curPet, 'idle')
        if (!row) continue
        setFrameIndex((prev) => {
          const next = prev + 1
          const isOneShot = curPet.oneShot.has(cur)
          if (isOneShot && !loopRef.current) {
            if (next >= row.frames) {
              if (!oneShotFiredRef.current) {
                oneShotFiredRef.current = true
                const cb = onOneShotEndRef.current
                if (cb) queueMicrotask(cb)
              }
              return row.frames - 1
            }
            return next
          }
          // Looping state: optionally pause on the last frame so the cycle
          // reads as a burst-then-rest rhythm instead of a continuous loop.
          if (next >= row.frames) {
            const restMs = loopRestMsFor(curPet, cur)
            if (restMs > 0) {
              restUntilRef.current = now + restMs
              return row.frames - 1
            }
            return 0
          }
          return next
        })
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => {
      cancelled = true
      cancelAnimationFrame(raf)
    }
  }, [])

  // Resolve the row to render. Pets that don't declare the requested
  // state fall back to idle to avoid blanks.
  const row = animationFor(pet, state) ?? animationFor(pet, 'idle')
  if (!row) return null
  const frame = Math.min(frameIndex, row.frames - 1)
  const offsetCol = row.offsetCol ?? 0
  const aspect = pet.atlas.cellH / pet.atlas.cellW
  // Keep each displayed atlas cell on whole CSS pixels. Fractional
  // background sizes/positions force subpixel sampling and make the pet
  // look soft, especially while the native window is moving.
  const renderW = Math.max(1, Math.round(size))
  const renderH = Math.max(1, Math.round(size * aspect))
  const totalW = renderW * pet.atlas.cols
  const totalH = renderH * pet.atlas.rows
  const bgX = -(offsetCol + frame) * renderW
  const bgY = -row.row * renderH

  // Per-row visual scale anchored at the feet: when a row's character
  // bbox is smaller than other rows (e.g. Yoonie's running pose is ~9%
  // shorter than her idle pose), declaring displayScale > 1 enlarges
  // just this row so the pet doesn't appear to shrink mid-animation.
  // transform-origin: bottom center keeps the feet on the floor.
  const rowScale = row.displayScale ?? 1
  const transformParts: string[] = []
  if (row.flipX) transformParts.push('scaleX(-1)')
  if (rowScale !== 1) transformParts.push(`scale(${rowScale})`)
  const transform = transformParts.length ? transformParts.join(' ') : undefined

  return (
    <div
      className={className}
      style={{
        width: renderW,
        height: renderH,
        backgroundImage: `url("${pet.spritesheetUrl}")`,
        backgroundRepeat: 'no-repeat',
        backgroundSize: `${totalW}px ${totalH}px`,
        backgroundPosition: `${bgX}px ${bgY}px`,
        imageRendering: pet.imageRendering,
        willChange: 'background-position',
        transform,
        transformOrigin: rowScale !== 1 ? 'bottom center' : undefined,
        overflow: 'visible',
        ...style,
      }}
    />
  )
}
