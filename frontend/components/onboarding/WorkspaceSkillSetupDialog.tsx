import { useEffect, useMemo, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Loader2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useSkillStore } from "@/stores/skill"
import { useWorkspaceStore } from "@/stores/workspace"

export function WorkspaceSkillSetupDialog() {
  const getLabel = useUiLabel()
  const open = useSkillStore((s) => s.skillSetupOpen)
  const closeSkillSetup = useSkillStore((s) => s.closeSkillSetup)
  const rootCatalog = useSkillStore((s) => s.rootCatalog)
  const loading = useSkillStore((s) => s.loading)
  const loadSkills = useSkillStore((s) => s.loadSkills)
  const enableRootSkill = useSkillStore((s) => s.enableRootSkill)
  const disableRootSkill = useSkillStore((s) => s.disableRootSkill)

  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId)
  const activeBinding = useWorkspaceStore((s) => s.activeBinding)

  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (open && activeBinding?.status === "bound") {
      closeSkillSetup()
      return
    }
    if (!open || !activeWorkspaceId) return
    void loadSkills(activeWorkspaceId)
    setError(null)
    setSelected(new Set())
  }, [open, activeWorkspaceId, activeBinding, loadSkills, closeSkillSetup])

  const skillIds = useMemo(
    () => rootCatalog.map((s) => s.id),
    [rootCatalog],
  )

  useEffect(() => {
    if (!open || loading || rootCatalog.length === 0) return
    setSelected(new Set(skillIds))
  }, [open, loading, rootCatalog.length, skillIds])

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const selectAll = () => setSelected(new Set(skillIds))
  const selectNone = () => setSelected(new Set())

  const handleSave = async () => {
    if (!activeWorkspaceId) return
    if (selected.size === 0) {
      setError("请至少选择一个 Skill")
      return
    }
    setSaving(true)
    setError(null)
    try {
      for (const skill of rootCatalog) {
        if (selected.has(skill.id)) {
          await enableRootSkill(activeWorkspaceId, skill.id)
        } else {
          await disableRootSkill(activeWorkspaceId, skill.id)
        }
      }
      closeSkillSetup()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next && !saving) closeSkillSetup()
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-90 bg-[rgba(58,44,31,0.42)] backdrop-blur-[2px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-91 flex max-h-[min(560px,calc(100vh-40px))] w-[min(520px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-panel border border-line bg-panel font-serif shadow-panel outline-none">
          <div className="shrink-0 border-b border-line px-5 py-4">
            <Dialog.Title className="text-[16px] text-ink">
              {getLabel("skill_setup.title", "为当前工作区启用 Skills")}
            </Dialog.Title>
            <Dialog.Description className="mt-1 text-[13px] text-muted-foreground">
              {getLabel(
                "skill_setup.desc",
                "从全局 Skill 目录中选择本工作区要启用的能力。之后可在 Skill 管理中调整。",
              )}
            </Dialog.Description>
          </div>

          <div className="min-h-0 flex-1 overflow-auto px-5 py-3">
            {loading ? (
              <div className="flex items-center justify-center gap-2 py-8 text-[13px] text-muted-foreground">
                <Loader2 className="size-4 animate-spin" />
                加载 Skill 目录…
              </div>
            ) : rootCatalog.length === 0 ? (
              <p className="py-8 text-center text-[13px] text-muted-foreground">
                全局目录暂无 Skill，可稍后在 Skill 管理中添加。
              </p>
            ) : (
              <ul className="space-y-2">
                {rootCatalog.map((skill) => (
                  <li key={skill.id}>
                    <label className="flex cursor-pointer items-start gap-3 rounded-[12px] border border-line bg-[rgba(255,255,255,0.42)] px-3 py-2.5 hover:bg-[rgba(255,255,255,0.62)]">
                      <input
                        type="checkbox"
                        className="mt-0.5"
                        checked={selected.has(skill.id)}
                        onChange={() => toggle(skill.id)}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="block text-[13px] font-semibold text-ink">
                          {skill.name}
                        </span>
                        {skill.description ? (
                          <span className="mt-0.5 block text-[12px] text-muted-foreground">
                            {skill.description}
                          </span>
                        ) : null}
                      </span>
                    </label>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {error ? (
            <p className="shrink-0 px-5 text-[12px] text-danger">{error}</p>
          ) : null}

          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-line px-5 py-4">
            <div className="flex gap-2">
              <Button type="button" variant="outline" size="sm" onClick={selectAll}>
                {getLabel("skill_setup.select_all", "全选")}
              </Button>
              <Button type="button" variant="outline" size="sm" onClick={selectNone}>
                {getLabel("skill_setup.select_none", "清空")}
              </Button>
            </div>
            <div className="flex gap-2">
              <Dialog.Close asChild>
                <Button type="button" variant="ghost" size="sm" disabled={saving}>
                  {getLabel("skill_setup.later", "稍后")}
                </Button>
              </Dialog.Close>
              <Button
                type="button"
                size="sm"
                disabled={saving || loading || rootCatalog.length === 0}
                onClick={() => void handleSave()}
              >
                {saving ? (
                  <>
                    <Loader2 className="mr-1 size-3.5 animate-spin" />
                    保存中…
                  </>
                ) : (
                  getLabel("skill_setup.save", "保存并继续")
                )}
              </Button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
