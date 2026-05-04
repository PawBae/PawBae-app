import { useCallback, useState } from 'react'
import { SpritePet } from './SpritePet'
import type { CodexPet, CodexPetState } from '../lib/codexPet'

interface MiniPetMascotProps {
  pet: CodexPet
  // Resting state computed by the parent: idle / running (working+compacting) /
  // waiting / run-right / run-left. `jumping` is owned by this wrapper via
  // hover and should not be passed in.
  baseState: CodexPetState
  size: number
  // When true, hovering plays jumping once before returning to baseState.
  // Set to false for non-interactive previews (session list, slots, settings).
  enableHoverJump?: boolean
  className?: string
  style?: React.CSSProperties
}

// Wraps SpritePet with hover-driven one-shot jumping. The base resting state
// is fully controlled by the parent so walking direction, working/waiting,
// etc. stay in one place upstream.
export function MiniPetMascot({
  pet,
  baseState,
  size,
  enableHoverJump = false,
  className,
  style,
}: MiniPetMascotProps) {
  const [jumping, setJumping] = useState(false)

  const onEnter = useCallback(() => {
    if (enableHoverJump) setJumping(true)
  }, [enableHoverJump])

  const onJumpEnd = useCallback(() => {
    setJumping(false)
  }, [])

  const renderState: CodexPetState = jumping ? 'jumping' : baseState

  return (
    <div
      className={className}
      onMouseEnter={enableHoverJump ? onEnter : undefined}
      style={{ display: 'inline-block', lineHeight: 0, ...style }}
    >
      <SpritePet
        pet={pet}
        state={renderState}
        size={size}
        onOneShotEnd={onJumpEnd}
      />
    </div>
  )
}
