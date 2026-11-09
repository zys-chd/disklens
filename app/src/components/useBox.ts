import { useEffect, useRef, useState } from 'react'

export function useBox<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const [box, setBox] = useState({ width: 0, height: 0 })

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const update = () => setBox({ width: el.clientWidth, height: el.clientHeight })
    update()
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', update)
      return () => window.removeEventListener('resize', update)
    }
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  return { ref, box }
}
