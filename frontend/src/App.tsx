import Mini from './Mini'
import { DemoMascot } from './DemoMascot'

function App() {
  // Demo mascot windows load `index.html#/mini?demo=1&pet=<id>` so they
  // share the bundle with the main mini window but render a stripped
  // mascot-only tree.
  const hash = typeof window !== 'undefined' ? window.location.hash : ''
  const isDemo = /[?&]demo=1\b/.test(hash)
  return isDemo ? <DemoMascot /> : <Mini />
}

export default App
