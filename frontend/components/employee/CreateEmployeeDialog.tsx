import { useCallback, useEffect, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Loader2, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { useUiLabel } from "@/hooks/useUiLabel"
import { useEmployeeStore } from "@/stores/employee"
import { useSkillStore } from "@/stores/skill"

export function CreateEmployeeDialog() {
  const t = useUiLabel()
  const open = useEmployeeStore((s) => s.createDialogOpen)
  const closeDialog = useEmployeeStore((s) => s.closeCreateDialog)
  const createEmployee = useEmployeeStore((s) => s.createEmployee)

  const rootCatalog = useSkillStore((s) => s.rootCatalog)
  const loadRootSkills = useSkillStore((s) => s.loadSkills)

  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const [initialPrompt, setInitialPrompt] = useState("")
  const [selectedSkillIds, setSelectedSkillIds] = useState<Set<string>>(new Set())
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setName("")
      setDescription("")
      setInitialPrompt("")
      setSelectedSkillIds(new Set())
      setError(null)
      void loadRootSkills()
    }
  }, [open, loadRootSkills])

  const toggleSkill = useCallback((skillId: string) => {
    setSelectedSkillIds((prev) => {
      const next = new Set(prev)
      if (next.has(skillId)) next.delete(skillId)
      else next.add(skillId)
      return next
    })
  }, [])

  const canSubmit = name.trim().length > 0 && !submitting

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return
    setSubmitting(true)
    setError(null)
    try {
      await createEmployee({
        name: name.trim(),
        description: description.trim() || undefined,
        initial_prompt: initialPrompt.trim() || undefined,
        root_skill_ids:
          selectedSkillIds.size > 0 ? Array.from(selectedSkillIds) : undefined,
      })
      setName("")
      setDescription("")
      setInitialPrompt("")
      setSelectedSkillIds(new Set())
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }, [
    canSubmit,
    name,
    description,
    initialPrompt,
    selectedSkillIds,
    createEmployee,
  ])

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) {
        closeDialog()
        setError(null)
      }
    },
    [closeDialog],
  )

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-90 bg-[rgba(36,40,50,0.28)]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-91 flex max-h-[min(680px,calc(100dvh-48px))] w-[min(620px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-white text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
          <div className="flex shrink-0 items-start justify-between gap-4 border-b border-line-soft px-[22px] py-5">
            <div className="min-w-0">
              <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
                {t("employee.create.kicker", "AI 数字员工")}
              </p>
              <Dialog.Title className="mt-[3px] text-[21px] font-extrabold tracking-normal">
                {t("employee.create.title", "新增 AI 员工")}
              </Dialog.Title>
              <Dialog.Description className="mt-1 text-[13px] text-muted-foreground">
                {t(
                  "employee.create.description",
                  "创建独立角色、方法来源和默认能力；员工 ID 由系统自动生成。",
                )}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                className="grid size-[38px] place-items-center rounded-[12px] border border-line bg-white text-muted-foreground transition-colors hover:bg-[#f8f9fb] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label={t("employee.create.cancel", "取消")}
              >
                <X className="size-4" />
              </button>
            </Dialog.Close>
          </div>

          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-[22px] py-[22px]">
            <div>
              <label className="block text-[13px] font-bold text-muted-foreground">
                {t("employee.create.field.name", "名称")}
              </label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("employee.create.name_placeholder", "例如：前端开发助手")}
                className="mt-1 min-h-[42px] rounded-[12px] border-line bg-white px-3 text-[13px]"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-[13px] font-bold text-muted-foreground">
                {t("employee.create.field.description", "描述（可选）")}
              </label>
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={2}
                placeholder={t("employee.create.description_placeholder", "这个员工的职责和能力…")}
                className="mt-1 min-h-[76px] rounded-[12px] border-line bg-white px-3 py-2 text-[13px]"
              />
            </div>
            <div>
              <label className="block text-[13px] font-bold text-muted-foreground">
                {t("employee.create.field.initial_prompt", "初始 Prompt（可选）")}
              </label>
              <Textarea
                value={initialPrompt}
                onChange={(e) => setInitialPrompt(e.target.value)}
                rows={4}
                placeholder={t("employee.create.prompt_placeholder", "员工默认系统 Prompt…")}
                className="mt-1 min-h-[112px] rounded-[12px] border-line bg-white px-3 py-2 font-mono text-[12px]"
              />
            </div>
            <div>
              <label className="block text-[13px] font-bold text-muted-foreground">
                {t("employee.create.field.root_skills", "从 Root 复制 Skills（可选）")}
              </label>
              {rootCatalog.length === 0 ? (
                <p className="mt-1 text-[11px] text-muted-foreground">
                  {t(
                    "employee.create.no_root_skills",
                    "暂无 Root Skill 可复制，创建后可在员工详情中添加",
                  )}
                </p>
              ) : (
                <ul className="mt-1 max-h-[140px] space-y-1 overflow-y-auto rounded-[14px] border border-line-soft bg-[#f8f9fb] p-2">
                  {rootCatalog.map((skill) => (
                    <li key={skill.id}>
                      <label className="flex cursor-pointer items-start gap-2 rounded-[10px] px-2 py-1.5 hover:bg-white">
                        <input
                          type="checkbox"
                          className="mt-0.5"
                          checked={selectedSkillIds.has(skill.id)}
                          onChange={() => toggleSkill(skill.id)}
                        />
                        <span className="min-w-0">
                          <span className="block text-[12px] font-medium text-ink">
                            {skill.name}
                          </span>
                          {skill.description ? (
                            <span className="block truncate text-[11px] text-muted-foreground">
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

            {error && (
              <p className="text-[12px] text-danger">{error}</p>
            )}
          </div>

          <div className="flex shrink-0 items-center justify-end gap-2 border-t border-line-soft px-[22px] py-5">
            <Dialog.Close asChild>
              <Button type="button" variant="ghost" size="sm" className="h-[36px] rounded-[12px] px-4">
                {t("employee.create.cancel", "取消")}
              </Button>
            </Dialog.Close>
            <Button
              type="button"
              size="sm"
              className="h-[36px] rounded-[12px] bg-primary px-4 font-bold text-primary-foreground hover:bg-primary/90"
              disabled={!canSubmit}
              onClick={() => void handleSubmit()}
            >
              {submitting ? (
                <>
                  <Loader2 className="mr-1 size-3 animate-spin" />
                  {t("employee.create.submitting", "创建中…")}
                </>
              ) : (
                t("employee.create.submit", "创建")
              )}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
