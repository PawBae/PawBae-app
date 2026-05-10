import { AnimatePresence, motion } from 'motion/react'

interface VoiceBubbleProps {
  visible: boolean
  text: string
  recording: boolean
  error?: string
  petMode?: boolean
}

export function VoiceBubble({ visible, text, recording, error, petMode }: VoiceBubbleProps) {
  const hasContent = text || recording || error
  const show = visible && hasContent

  if (!show) return null

  // Efficiency mode: compact pulsing dot indicator
  if (!petMode) {
    return (
      <AnimatePresence>
        {recording && (
          <motion.div
            initial={{ opacity: 0, scale: 0 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0 }}
            style={{
              position: 'fixed',
              top: 2,
              right: 2,
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: '#F5A623',
              boxShadow: '0 0 6px rgba(245,166,35,0.6)',
              animation: 'voicePulse 1.2s ease-in-out infinite',
              pointerEvents: 'none',
              zIndex: 99999,
            }}
          >
            <style>{`@keyframes voicePulse { 0%, 100% { transform: scale(1); opacity: 1; } 50% { transform: scale(1.3); opacity: 0.7; } }`}</style>
          </motion.div>
        )}
      </AnimatePresence>
    )
  }

  // Pet mode: full bubble with text
  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ opacity: 0, y: 8, scale: 0.9 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 8, scale: 0.9 }}
          transition={{ type: 'spring', stiffness: 400, damping: 25 }}
          style={{
            position: 'absolute',
            top: 0,
            left: '50%',
            transform: 'translateX(-50%)',
            pointerEvents: 'none',
            zIndex: 9999,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
          }}
        >
          <div
            style={{
              background: error ? '#e74c3c' : '#F5A623',
              borderRadius: 18,
              padding: '6px 14px',
              color: '#fff',
              fontSize: 13,
              fontWeight: 500,
              maxWidth: 280,
              minWidth: 40,
              textAlign: 'center',
              wordBreak: 'break-word',
              lineHeight: 1.4,
              whiteSpace: 'pre-wrap',
              boxShadow: '0 2px 8px rgba(0,0,0,0.3)',
            }}
          >
            {error ? (
              error
            ) : text ? (
              <>
                {text}
                {recording && <BlinkCursor />}
              </>
            ) : (
              recording && <BlinkCursor />
            )}
          </div>
          {/* Triangle pointer */}
          <div
            style={{
              width: 0,
              height: 0,
              borderLeft: '6px solid transparent',
              borderRight: '6px solid transparent',
              borderTop: `6px solid ${error ? '#e74c3c' : '#F5A623'}`,
            }}
          />
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function BlinkCursor() {
  return (
    <span style={{ marginLeft: 1, fontWeight: 300, animation: 'voiceBlink 1s step-end infinite' }}>
      |
      <style>{`@keyframes voiceBlink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }`}</style>
    </span>
  )
}
