import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import type { SkillEffectiveMode, SkillSummary } from "@/types/skill"

export interface WorkspaceSkillListProps {
  selection: SkillSummary[]
  local: SkillSummary[]
  workspaceId: string | null
  onDelete: (skillId: string) => void
}

function modeBadge(mode: SkillEffectiveMode): { label: string; className: string } {
  if (mode === "workspace_override") {
    return {
      label: "覆盖 root",
      className: "border-warning/30 bg-warning/10 text-warning",
    }
  }
  if (mode === "workspace_local") {
    return { label: "本地", className: "border-accent/30 bg-accent/10 text-accent-dark" }
  }
  if (mode === "selected_root") {
    return { label: "已选 root", className: "border-success/25 bg-success/10 text-success" }
  }
  return { label: mode, className: "border-line text-muted-foreground" }
}

function SkillRow({
  skill,
  workspaceId,
  onDelete,
}: {
  skill: SkillSummary
  workspaceId: string | null
  onDelete: (skillId: string) => void
}) {
  const badge = modeBadge(skill.effective_mode)

  return (
    <li className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <span className="block text-[13px] font-semibold text-ink">{skill.name}</span>
          <span className="mt-0.5 block truncate text-[12px] text-muted-foreground">
            {skill.description || "无描述"}
          </span>
        </div>
        <Badge variant="outline" className={cn("shrink-0 font-sans text-[11px]", badge.className)}>
          {badge.label}
        </Badge>
      </div>
      <div className="mt-2 flex flex-wrap gap-2">
        <Button
          type="button"
          size="xs"
          variant="destructive"
          className="rounded-[10px]"
          disabled={!workspaceId}
          onClick={() => onDelete(skill.id)}
        >
          删除
        </Button>
        <Button
          type="button"
          size="xs"
          variant="outline"
          className="rounded-[10px] bg-white"
          disabled
          title="将在 Review 流程中实现"
        >
          推广到全局
        </Button>
      </div>
    </li>
  )
}

export function WorkspaceSkillList({
  selection,
  local,
  workspaceId,
  onDelete,
}: WorkspaceSkillListProps) {
  const all = [...selection, ...local]
  const seen = new Set<string>()
  const rows = all.filter((s) => {
    if (seen.has(s.id)) return false
    seen.add(s.id)
    return true
  })

  if (rows.length === 0) {
    return (
      <p className="px-2 py-4 text-[13px] text-muted-foreground">当前工作区暂无 Skill 选择或本地项</p>
    )
  }

  return (
    <ScrollArea className="h-[min(360px,50vh)] pr-3">
      <ul className="grid gap-2">
        {rows.map((skill) => (
          <SkillRow
            key={skill.id}
            skill={skill}
            workspaceId={workspaceId}
            onDelete={onDelete}
          />
        ))}
      </ul>
    </ScrollArea>
  )
}
