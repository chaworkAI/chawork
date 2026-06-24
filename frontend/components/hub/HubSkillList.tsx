import { useEffect, useRef, type RefObject } from "react"
import { ChevronDown, Download, Loader2, RefreshCw, Trash2 } from "lucide-react"

import { HubCardFeedback } from "@/components/hub/HubCardFeedback"
import {
  getHubRelation,
  isGithubHubSource,
  relationBadgeClass,
} from "@/components/hub/hubLocalState"
import { Button } from "@/components/ui/button"
import type { HubInstallFeedback } from "@/stores/hub"
import type { HubDownloadFilter, HubSkillView } from "@/types/hub"

function skillDescription(skill: HubSkillView) {
  return skill.description_zh || skill.description_en || ""
}

function installLabel(skill: HubSkillView) {
  if (skill.update_available) return "更新"
  if (skill.downloaded || skill.local_source != null) return "覆盖"
  return "下载"
}

export function HubSkillList({
  skills,
  filter,
  installingIds,
  loadingMore,
  hasMore,
  feedbackByKey,
  scrollRoot,
  onLoadMore,
  onInstall,
  onDelete,
}: {
  skills: HubSkillView[]
  filter: HubDownloadFilter
  installingIds: string[]
  loadingMore: boolean
  hasMore: boolean
  feedbackByKey: Record<string, HubInstallFeedback>
  scrollRoot?: RefObject<HTMLElement | null>
  onLoadMore: () => void
  onInstall: (id: string) => void
  onDelete: (id: string, installed: boolean) => void
}) {
  const loadMoreSentinelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!hasMore || loadingMore) return

    const sentinel = loadMoreSentinelRef.current
    if (!sentinel) return

    const root = scrollRoot?.current ?? null
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          onLoadMore()
        }
      },
      { root, rootMargin: "160px", threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [hasMore, loadingMore, onLoadMore, scrollRoot])
  if (skills.length === 0) {
    return (
      <p className="py-10 text-center text-[13px] text-muted-foreground">
        没有匹配的技能
      </p>
    )
  }

  return (
    <div>
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {skills.map((skill) => {
          const installing = installingIds.includes(skill.id)
          const busy = installing
          const relation = getHubRelation(skill, "skill", {
            source: skill.source,
            isUserCustomImport: isGithubHubSource(skill.source),
          })
          const showDelete = filter === "local" || filter === "custom"
          const showInstall = filter !== "local" && !skill.downloaded
          const deleteInstalled = filter === "local" ? true : skill.downloaded

          return (
            <div
              key={skill.id}
              className="relative grid min-h-[168px] grid-rows-[1fr_auto] overflow-hidden rounded-[8px] border border-line-soft bg-white p-4"
            >
              <HubCardFeedback feedback={feedbackByKey[`skills:${skill.id}`]} />
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="min-w-0 truncate text-[14px] font-extrabold text-ink">
                    {skill.name}
                  </p>
                  <span className="rounded-[8px] bg-[#eef1f5] px-2 py-0.5 text-[10px] font-bold text-muted-foreground">
                    {skill.profession}
                  </span>
                  <span
                    className={[
                      "rounded-[8px] px-2 py-0.5 text-[10px] font-bold",
                      relationBadgeClass(relation.tone),
                    ].join(" ")}
                  >
                    {relation.label}
                  </span>
                </div>
                {skillDescription(skill) ? (
                  <p className="mt-2 line-clamp-3 text-[12px] leading-5 text-muted-foreground">
                    {skillDescription(skill)}
                  </p>
                ) : null}
                <p className="mt-2 truncate font-mono text-[10px] text-muted-foreground">
                  {skill.id}
                </p>
                {skill.tags.length > 0 ? (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {skill.tags.slice(0, 4).map((tag) => (
                      <span
                        key={tag}
                        className="rounded-[7px] bg-[#f4f6f8] px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                ) : null}
              </div>
              <div className="mt-3 flex items-center justify-end gap-2">
                {showInstall ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={busy || relation.actionBlocked}
                    onClick={() => onInstall(skill.id)}
                  >
                    {busy ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : skill.update_available || skill.downloaded || skill.local_source != null ? (
                      <RefreshCw className="size-3" />
                    ) : (
                      <Download className="size-3" />
                    )}
                    {installLabel(skill)}
                  </Button>
                ) : null}
                {showDelete ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onClick={() => {
                      const message = deleteInstalled
                        ? `确认删除技能「${skill.name}」？将同时从本地和自定义列表移除。`
                        : `确认从自定义列表移除「${skill.name}」？`
                      if (!window.confirm(message)) {
                        return
                      }
                      onDelete(skill.id, deleteInstalled)
                    }}
                  >
                    {busy ? (
                      <Loader2 className="size-3 animate-spin" />
                    ) : (
                      <Trash2 className="size-3" />
                    )}
                    删除
                  </Button>
                ) : null}
              </div>
            </div>
          )
        })}
      </div>
      <div ref={loadMoreSentinelRef} className="flex justify-center py-4">
        {hasMore ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={loadingMore}
            onClick={onLoadMore}
          >
            {loadingMore ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <ChevronDown className="size-3" />
            )}
            加载更多
          </Button>
        ) : (
          <p className="text-[12px] text-muted-foreground">已全部加载</p>
        )}
      </div>
    </div>
  )
}
