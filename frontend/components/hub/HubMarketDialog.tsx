import { useCallback, useMemo, useRef } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { Link2, Loader2, RefreshCw, Search, X } from "lucide-react"

import { HubEmployeeList } from "@/components/hub/HubEmployeeList"
import { HubFilterBar } from "@/components/hub/HubFilterBar"
import { HubSkillList } from "@/components/hub/HubSkillList"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useHubStore, type HubTab } from "@/stores/hub"
import type {
  HubDownloadFilter,
  HubEmployeeView,
  HubLocalState,
  HubSkillView,
} from "@/types/hub"

const TABS: Array<{ key: HubTab; label: string }> = [
  { key: "skills", label: "技能" },
  { key: "employees", label: "员工" },
]

function matchesRelationFilter(item: HubLocalState, filter: HubDownloadFilter) {
  if (filter === "all") return true
  if (filter === "remote") return !item.downloaded && item.local_source == null
  if (filter === "local") return item.downloaded && !item.update_available
  if (filter === "update_available") return item.update_available
  if (filter === "custom") return !item.downloaded && item.local_source != null
  return true
}

function matchesEmployeeFilter(
  employee: HubEmployeeView,
  filter: HubDownloadFilter,
  userCustomGithubEmployeeIds: string[],
) {
  if (filter === "custom") {
    return userCustomGithubEmployeeIds.includes(employee.id)
  }
  return matchesRelationFilter(employee, filter)
}

function matchesSkillFilter(
  skill: HubSkillView,
  filter: HubDownloadFilter,
) {
  if (filter === "remote") {
    return !skill.downloaded && skill.local_source == null
  }
  if (filter === "custom" || filter === "local" || filter === "update_available") {
    return true
  }
  return matchesRelationFilter(skill, filter)
}

function normalizedQuery(query: string) {
  return query.trim().toLocaleLowerCase()
}

function textIncludes(value: string | undefined, query: string) {
  return value?.toLocaleLowerCase().includes(query) ?? false
}

function skillMatchesQuery(skill: HubSkillView, query: string) {
  if (!query) return true
  return (
    textIncludes(skill.id, query) ||
    textIncludes(skill.name, query) ||
    textIncludes(skill.profession, query) ||
    textIncludes(skill.description_zh, query) ||
    textIncludes(skill.description_en, query) ||
    skill.tags.some((tag) => textIncludes(tag, query))
  )
}

function employeeMatchesQuery(employee: HubEmployeeView, query: string) {
  if (!query) return true
  return (
    textIncludes(employee.id, query) ||
    textIncludes(employee.name, query) ||
    textIncludes(employee.description, query) ||
    employee.tags.some((tag) => textIncludes(tag, query)) ||
    employee.skill_ids.some((skillId) => textIncludes(skillId, query))
  )
}

export function HubMarketDialog() {
  const open = useHubStore((s) => s.marketOpen)
  const close = useHubStore((s) => s.closeMarket)
  const activeTab = useHubStore((s) => s.activeTab)
  const setActiveTab = useHubStore((s) => s.setActiveTab)
  const skillsFilter = useHubStore((s) => s.skillsFilter)
  const employeesFilter = useHubStore((s) => s.employeesFilter)
  const setSkillsFilter = useHubStore((s) => s.setSkillsFilter)
  const setEmployeesFilter = useHubStore((s) => s.setEmployeesFilter)
  const filter = activeTab === "skills" ? skillsFilter : employeesFilter
  const setFilter =
    activeTab === "skills" ? setSkillsFilter : setEmployeesFilter
  const query = useHubStore((s) => s.query)
  const setQuery = useHubStore((s) => s.setQuery)
  const skills = useHubStore((s) => s.skills)
  const skillsTotal = useHubStore((s) => s.skillsTotal)
  const skillsLoadingMore = useHubStore((s) => s.skillsLoadingMore)
  const loadMoreSkills = useHubStore((s) => s.loadMoreSkills)
  const employees = useHubStore((s) => s.employees)
  const loading = useHubStore((s) => s.loading)
  const error = useHubStore((s) => s.error)
  const manifest = useHubStore((s) => s.manifest)
  const installingIds = useHubStore((s) => s.installingIds)
  const installFeedback = useHubStore((s) => s.installFeedback)
  const reloadActive = useHubStore((s) => s.reloadActive)
  const installSkill = useHubStore((s) => s.installSkill)
  const deleteSkill = useHubStore((s) => s.deleteSkill)
  const installEmployee = useHubStore((s) => s.installEmployee)
  const deleteEmployee = useHubStore((s) => s.deleteEmployee)
  const openGithubImport = useHubStore((s) => s.openGithubImport)
  const userCustomGithubEmployeeIds = useHubStore((s) => s.userCustomGithubEmployeeIds)

  const handleRefresh = useCallback(() => {
    void reloadActive()
  }, [reloadActive])

  const queryText = normalizedQuery(query)
  const visibleSkills = useMemo(
    () =>
      skills.filter(
        (skill) =>
          matchesSkillFilter(skill, skillsFilter) &&
          skillMatchesQuery(skill, queryText),
      ),
    [skills, skillsFilter, queryText],
  )
  const visibleEmployees = useMemo(
    () =>
      employees.filter(
        (employee) =>
          (matchesEmployeeFilter(
            employee,
            employeesFilter,
            userCustomGithubEmployeeIds,
          ) ||
            installFeedback[`employees:${employee.id}`]) &&
          employeeMatchesQuery(employee, queryText),
      ),
    [
      employees,
      employeesFilter,
      installFeedback,
      queryText,
      userCustomGithubEmployeeIds,
    ],
  )
  const activeListLoading =
    loading &&
    (activeTab === "skills" ? visibleSkills.length === 0 : visibleEmployees.length === 0)
  const hasMoreSkills = skills.length < skillsTotal
  const skillsScrollRef = useRef<HTMLDivElement>(null)

  const handleLoadMoreSkills = useCallback(() => {
    void loadMoreSkills()
  }, [loadMoreSkills])

  const handleOpenAutoFocus = useCallback((event: Event) => {
    event.preventDefault()
    window.requestAnimationFrame(() => {
      document.getElementById("hub-market-search")?.focus()
    })
  }, [])

  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && close()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[80] bg-transparent backdrop-blur-[2px]" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-[81] grid h-[min(88vh,860px)] w-[min(1220px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 grid-rows-[auto_1fr] overflow-hidden rounded-[18px] border border-line bg-panel shadow-2xl outline-none"
          onOpenAutoFocus={handleOpenAutoFocus}
        >
          <header className="border-b border-line px-6 py-5">
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-[11px] font-extrabold uppercase text-muted-foreground">
                  Skill Hub
                </p>
                <Dialog.Title className="mt-1 text-[24px] font-black text-ink">
                  {activeTab === "skills" ? "技能市场" : "员工市场"}
                </Dialog.Title>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {activeTab === "skills" ? (
                  <Button
                    type="button"
                    size="lg"
                    onClick={(event) => {
                      event.preventDefault()
                      event.stopPropagation()
                      openGithubImport()
                    }}
                  >
                    <Link2 className="size-4" />
                    从 GitHub 导入
                  </Button>
                ) : null}
                <div className="inline-flex rounded-[12px] border border-line bg-[#f6f7f9] p-1">
                  {TABS.map((tab) => (
                    <button
                      key={tab.key}
                      type="button"
                      className={[
                        "h-8 rounded-[9px] px-4 text-[13px] font-bold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/25",
                        activeTab === tab.key
                          ? "bg-white text-ink shadow-sm"
                          : "text-muted-foreground hover:text-ink",
                      ].join(" ")}
                      onClick={() => setActiveTab(tab.key)}
                    >
                      {tab.label}
                    </button>
                  ))}
                </div>
                <Button type="button" variant="outline" size="icon-lg" onClick={close}>
                  <X className="size-4" />
                </Button>
              </div>
            </div>
          </header>

          <main className="grid min-h-0 grid-rows-[auto_1fr]">
            <div className="border-b border-line-soft bg-panel px-6 py-4">
              <div className="grid gap-3 lg:grid-cols-[1fr_auto_auto]">
                <div className="relative">
                  <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="hub-market-search"
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") event.preventDefault()
                    }}
                    placeholder={activeTab === "skills" ? "搜索技能" : "搜索员工"}
                    className="h-10 rounded-[12px] border-line bg-white pl-9 text-[13px]"
                  />
                </div>
                <HubFilterBar value={filter} onChange={setFilter} />
                <Button type="button" variant="outline" size="lg" onClick={handleRefresh}>
                  {loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                  刷新
                </Button>
              </div>

            {manifest ? (
              <p className="mt-3 text-[12px] text-muted-foreground">
                Hub 收录 {manifest.total_skills} 个技能，{manifest.total_employees} 个员工
              </p>
            ) : null}
            {error ? (
              <div className="mt-3 rounded-[12px] border border-[#f0b8b8] bg-[#fff1f1] px-3 py-2 text-[12px] text-[#8f2424]">
                {error}
              </div>
            ) : null}
            </div>

            <div ref={skillsScrollRef} className="min-h-0 overflow-y-auto px-6 py-5">
              {error ? null : activeListLoading ? (
                <div className="flex min-h-[180px] items-center justify-center gap-2 text-[13px] text-muted-foreground">
                  <Loader2 className="size-4 animate-spin" />
                  加载中
                </div>
              ) : activeTab === "skills" ? (
                  <HubSkillList
                    skills={visibleSkills}
                    filter={skillsFilter}
                    installingIds={installingIds}
                    loadingMore={skillsLoadingMore}
                    feedbackByKey={installFeedback}
                    hasMore={hasMoreSkills}
                    scrollRoot={skillsScrollRef}
                    onLoadMore={handleLoadMoreSkills}
                    onInstall={(id) => void installSkill(id)}
                    onDelete={(id, installed) => void deleteSkill(id, { installed })}
                />
              ) : (
                  <HubEmployeeList
                    employees={visibleEmployees}
                    filter={employeesFilter}
                    userCustomGithubEmployeeIds={userCustomGithubEmployeeIds}
                    installingIds={installingIds}
                    feedbackByKey={installFeedback}
                    onInstall={(id) => void installEmployee(id)}
                    onDelete={(id, installed) => void deleteEmployee(id, { installed })}
                  />
              )}
            </div>
          </main>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
