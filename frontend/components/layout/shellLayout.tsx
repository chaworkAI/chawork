import { createContext, useContext, useEffect, useState, type ReactNode } from "react"

export type ShellLayout = "wide" | "tablet" | "mobile"

const ShellLayoutContext = createContext<ShellLayout>("wide")

function resolveLayout(width: number): ShellLayout {
  if (width >= 1200) return "wide"
  if (width >= 900) return "tablet"
  return "mobile"
}

export function useShellLayout(): ShellLayout {
  return useContext(ShellLayoutContext)
}

export function ShellLayoutProvider({
  children,
  layout,
}: {
  children: ReactNode
  layout: ShellLayout
}) {
  return (
    <ShellLayoutContext.Provider value={layout}>{children}</ShellLayoutContext.Provider>
  )
}

export function useShellBreakpoint(): ShellLayout {
  const [layout, setLayout] = useState<ShellLayout>(() =>
    typeof window !== "undefined" ? resolveLayout(window.innerWidth) : "wide",
  )

  useEffect(() => {
    const onResize = () => setLayout(resolveLayout(window.innerWidth))
    onResize()
    window.addEventListener("resize", onResize)
    return () => window.removeEventListener("resize", onResize)
  }, [])

  return layout
}
