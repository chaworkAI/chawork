import * as Dialog from "@radix-ui/react-dialog"
import { useEffect, useMemo } from "react"
import { AlertTriangle, X } from "lucide-react"

import { RootSkillList } from "@/components/skills/RootSkillList"
import { WorkspaceSkillList } from "@/components/skills/WorkspaceSkillList"
import { Button } from "@/components/ui/button"
import { useMcpToolStore } from "@/stores/mcpTool"
import { useRootConfigStore } from "@/stores/rootConfig"
import { useSkillStore } from "@/stores/skill"
import { useWorkspaceStore } from "@/stores/workspace"

export function SkillManager() {
  const open = useSkillStore((s) => s.skillManagerOpen)
  const closeSkillManager = useSkillStore((s) => s.closeSkillManager)
  const rootCatalog = useSkillStore((s) => s.rootCatalog)
  const workspaceSelection = useSkillStore((s) => s.workspaceSelection)
  const workspaceLocal = useSkillStore((s) => s.workspaceLocal)
  const loading = useSkillStore((s) => s.loading)
  const error = useSkillStore((s) => s.error)
  const loadSkills = useSkillStore((s) => s.loadSkills)
  const enableRootSkill = useSkillStore((s) => s.enableRootSkill)
  const disableRootSkill = useSkillStore((s) => s.disableRootSkill)
  const createWorkspaceOverride = useSkillStore((s) => s.createWorkspaceOverride)
  const deleteWorkspaceSkill = useSkillStore((s) => s.deleteWorkspaceSkill)

  const rootInfo = useRootConfigStore((s) => s.rootInfo)
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId)
  const workspaces = useWorkspaceStore((s) => s.workspaces)

  const mcpPolicy = useMcpToolStore((s) => s.policy)
  const loadToolPolicy = useMcpToolStore((s) => s.loadToolPolicy)
  const openWorkspaceConfig = useWorkspaceStore((s) => s.openWorkspaceConfig)

  const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId)

  useEffect(() => {
    if (!open) return
    void loadSkills(activeWorkspaceId ?? undefined)
    if (activeWorkspaceId) {
      void loadToolPolicy(activeWorkspaceId)
    }
  }, [open, activeWorkspaceId, loadSkills, loadToolPolicy])

  const disabledTools = useMemo(() => {
    if (!mcpPolicy?.tools) return new Set<string>()
    return new Set(
      mcpPolicy.tools.filter((t) => !t.enabled).map((t) => t.id),
    )
  }, [mcpPolicy])

  const skillsWithMissingTools = useMemo(() => {
    const allSkills = [...rootCatalog, ...workspaceLocal]
    return allSkills.filter(
      (s) => s.enabled && s.depends_on_tools?.some((t) => disabledTools.has(t)),
    )
  }, [rootCatalog, workspaceLocal, disabledTools])

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && closeSkillManager()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)]" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-81 flex max-h-[min(760px,calc(100dvh-48px))] w-[min(920px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none"
        >
          <header className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
            <div className="min-w-0">
              <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                技能与工具
              </p>
              <Dialog.Title className="mt-[3px] text-[21px] font-extrabold tracking-normal text-ink">Skill 管理</Dialog.Title>
              <Dialog.Description className="mt-1 text-[13px] text-muted-foreground">
                管理工作区与 Root Skill 启用状态（已绑定员工时请使用员工管理）
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                className="grid size-[38px] place-items-center rounded-[12px] border border-line bg-white text-muted-foreground transition-colors hover:bg-[#f8f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label="关闭"
              >
                <X className="size-4" />
              </button>
            </Dialog.Close>
          </header>

          <div className="shrink-0 space-y-1 border-b border-line-soft px-[22px] py-3 text-[13px] text-muted-foreground">
            <p>
              <span className="font-medium text-ink">根工作区：</span>
              {rootInfo?.path || "未初始化"}
            </p>
            <p>
              <span className="font-medium text-ink">当前子工作区：</span>
              {activeWorkspace?.path || "未选择"}
            </p>
            {error ? <p className="text-danger">{error}</p> : null}
          </div>

          <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 overflow-hidden px-[22px] py-[22px] min-[720px]:grid-cols-2">
            <section className="flex min-h-0 flex-col gap-2">
              <h3 className="text-[14px] font-bold text-ink">
                Root Skill Catalog
              </h3>
              <RootSkillList
                skills={rootCatalog}
                workspaceId={activeWorkspaceId}
                loading={loading}
                onEnable={(id) => {
                  if (!activeWorkspaceId) return
                  void enableRootSkill(activeWorkspaceId, id)
                }}
                onDisable={(id) => {
                  if (!activeWorkspaceId) return
                  void disableRootSkill(activeWorkspaceId, id)
                }}
                onCreateOverride={(id) => {
                  if (!activeWorkspaceId) return
                  void createWorkspaceOverride(activeWorkspaceId, id)
                }}
              />
            </section>

            <section className="flex min-h-0 flex-col gap-2">
              <h3 className="text-[14px] font-bold text-ink">
                当前子工作区 Skills
              </h3>
              <WorkspaceSkillList
                selection={workspaceSelection}
                local={workspaceLocal}
                workspaceId={activeWorkspaceId}
                onDelete={(skillId) => {
                  if (!activeWorkspaceId) return
                  void deleteWorkspaceSkill(activeWorkspaceId, skillId)
                }}
              />
            </section>
          </div>

          {skillsWithMissingTools.length > 0 ? (
            <div className="shrink-0 border-t border-warning/30 bg-warning/5 px-5 py-3">
              <div className="flex items-start gap-2 text-[12px] text-warning">
                <AlertTriangle className="mt-0.5 size-4 shrink-0" />
                <div className="space-y-1">
                  <p className="font-medium">部分 Skill 依赖的工具已在当前工作区关闭</p>
                  <ul className="list-inside list-disc text-muted-foreground">
                    {skillsWithMissingTools.map((s) => (
                      <li key={s.id}>{s.name}</li>
                    ))}
                  </ul>
                  <Button
                    type="button"
                    size="xs"
                    variant="outline"
                    className="mt-1 h-[34px] rounded-[12px] bg-white px-3 text-[12px]"
                    onClick={() => {
                      closeSkillManager()
                      openWorkspaceConfig("tools")
                    }}
                  >
                    打开工具配置
                  </Button>
                </div>
              </div>
            </div>
          ) : null}

          <footer className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t border-line-soft px-[22px] py-5">
            <p className="text-[12px] text-muted-foreground">
              Skill 配置保存后会用于后续消息。
            </p>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
