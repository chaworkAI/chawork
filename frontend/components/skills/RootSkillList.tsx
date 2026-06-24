import { useState } from "react"
import { ChevronDown, ChevronRight } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import type { SkillSummary } from "@/types/skill"

export interface RootSkillListProps {
  skills: SkillSummary[]
  workspaceId: string | null
  loading?: boolean
  onEnable: (rootSkillId: string) => void
  onDisable: (rootSkillId: string) => void
  onCreateOverride: (rootSkillId: string) => void
}

function isEnabled(skill: SkillSummary): boolean {
  return skill.effective_mode === "selected_root"
}

export function RootSkillList({
  skills,
  workspaceId,
  loading,
  onEnable,
  onDisable,
  onCreateOverride,
}: RootSkillListProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null)

  if (loading) {
    return <p className="px-2 py-4 text-[13px] text-muted-foreground">加载中…</p>
  }

  if (skills.length === 0) {
    return <p className="px-2 py-4 text-[13px] text-muted-foreground">根目录暂无 Skill</p>
  }

  return (
    <ScrollArea className="h-[min(360px,50vh)] pr-3">
      <ul className="grid gap-2">
        {skills.map((skill) => {
          const enabled = isEnabled(skill)
          const expanded = expandedId === skill.id
          return (
            <li
              key={skill.id}
              className="rounded-[15px] border border-line-soft bg-[#f8f9fb] px-3.5 py-3"
            >
              <div className="flex items-start gap-2">
                <button
                  type="button"
                  className="mt-0.5 shrink-0 text-muted-foreground"
                  aria-expanded={expanded}
                  onClick={() => setExpandedId(expanded ? null : skill.id)}
                >
                  {expanded ? (
                    <ChevronDown className="size-4" />
                  ) : (
                    <ChevronRight className="size-4" />
                  )}
                </button>
                <div className="min-w-0 flex-1">
                  <button
                    type="button"
                    className="text-left"
                    onClick={() => setExpandedId(expanded ? null : skill.id)}
                  >
                    <span className="block text-[13px] font-semibold text-ink">{skill.name}</span>
                    <span className="mt-0.5 block truncate text-[12px] text-muted-foreground">
                      {skill.description || "无描述"}
                    </span>
                  </button>
                </div>
                <Badge
                  variant="outline"
                  className={cn(
                    "shrink-0 font-sans text-[11px]",
                    enabled
                      ? "border-success/25 bg-success/10 text-success"
                      : "border-line text-muted-foreground",
                  )}
                >
                  {enabled ? "已启用" : "未启用"}
                </Badge>
              </div>
              {expanded ? (
                <dl className="mt-2 space-y-1 border-t border-line/80 pt-2 pl-6 text-[12px] text-muted-foreground">
                  <div>
                    <dt className="inline font-medium text-ink">描述：</dt>
                    <dd className="inline">{skill.description || "—"}</dd>
                  </div>
                  <div>
                    <dt className="inline font-medium text-ink">路径：</dt>
                    <dd className="inline break-all">{skill.path}</dd>
                  </div>
                  <div>
                    <dt className="inline font-medium text-ink">版本：</dt>
                    <dd className="inline">{skill.version ?? "—"}</dd>
                  </div>
                  <div>
                    <dt className="inline font-medium text-ink">校验和：</dt>
                    <dd className="inline break-all font-mono text-[11px]">{skill.checksum}</dd>
                  </div>
                </dl>
              ) : null}
              <div className="mt-2 flex flex-wrap gap-2 pl-6">
                {!workspaceId ? (
                  <span className="text-[12px] text-muted-foreground">请先选择工作区</span>
                ) : enabled ? (
                  <>
                    <Button
                      type="button"
                      size="xs"
                      variant="outline"
                      className="rounded-[10px] bg-white"
                      onClick={() => onDisable(skill.id)}
                    >
                      停用
                    </Button>
                    <Button
                      type="button"
                      size="xs"
                      variant="secondary"
                      className="rounded-[10px]"
                      onClick={() => onCreateOverride(skill.id)}
                    >
                      创建本地覆盖
                    </Button>
                  </>
                ) : (
                  <Button
                    type="button"
                    size="xs"
                    className="rounded-[10px]"
                    onClick={() => onEnable(skill.id)}
                  >
                    启用
                  </Button>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </ScrollArea>
  )
}
