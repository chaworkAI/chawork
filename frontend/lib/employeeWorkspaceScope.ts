import { normalizePathKey, pathKeysEqual } from "@/lib/formatPath"
import type { WorkspaceMembership } from "@/types/employee"
import type { WorkspaceSidebarItem } from "@/types/workspace"

function findItemByPath(
  items: WorkspaceSidebarItem[],
  workspacePath: string,
): WorkspaceSidebarItem | undefined {
  return items.find((item) => pathKeysEqual(item.workspace.path, workspacePath))
}

function dedupeByPath(items: WorkspaceSidebarItem[]): WorkspaceSidebarItem[] {
  const seen = new Set<string>()
  return items.filter((item) => {
    const key = normalizePathKey(item.workspace.path)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

function sidebarItemFromMembership(
  membership: WorkspaceMembership,
  existing: WorkspaceSidebarItem | undefined,
  contextEmployeeId: string,
): WorkspaceSidebarItem {
  if (existing) {
    if (existing.workspace.bound_employee_id === contextEmployeeId) {
      return existing
    }
    return {
      ...existing,
      workspace: {
        ...existing.workspace,
        bound_employee_id: contextEmployeeId,
      },
    }
  }

  return {
    workspace: {
      id: membership.id,
      name: membership.name,
      path: membership.path,
      created_at: membership.added_at,
      last_active_at: membership.added_at,
      active_session_id: null,
      domain_pack_id: null,
      index_status: "stale",
      bound_employee_id: contextEmployeeId,
      bound_employee_name: null,
    },
    metaLine: membership.path,
  }
}

/** Sidebar rows scoped to one employee's bound workspace folders. */
export function sidebarItemsForEmployee(
  items: WorkspaceSidebarItem[],
  contextEmployeeId: string | null,
  memberships: WorkspaceMembership[],
  activeWorkspacePath?: string | null,
): WorkspaceSidebarItem[] {
  if (!contextEmployeeId) {
    if (!activeWorkspacePath) return []
    const activeItem = findItemByPath(items, activeWorkspacePath)
    return activeItem ? [activeItem] : []
  }

  const memberPathKeys = new Set(memberships.map((m) => normalizePathKey(m.path)))

  const fromMemberships = memberships.map((membership) =>
    sidebarItemFromMembership(
      membership,
      findItemByPath(items, membership.path),
      contextEmployeeId,
    ),
  )

  const extraFromRegistry = items.filter((item) => {
    if (item.workspace.bound_employee_id !== contextEmployeeId) return false
    return !memberPathKeys.has(normalizePathKey(item.workspace.path))
  })

  return dedupeByPath([...fromMemberships, ...extraFromRegistry])
}
