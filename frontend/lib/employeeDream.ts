import type { RegistryEntry } from "@/types/employee"

/** Built-in Dream executor employee id. */
export const DREAM_WORKFLOW_EMPLOYEE_ID = "__dream__"

export function isDreamWorkflowEmployee(
  entry: Pick<RegistryEntry, "id" | "kind"> | null | undefined,
): boolean {
  return entry?.kind === "dream" || entry?.id === DREAM_WORKFLOW_EMPLOYEE_ID
}

export function isOrdinaryEmployee(
  entry: Pick<RegistryEntry, "kind"> | null | undefined,
): boolean {
  return entry?.kind === "ordinary"
}

/** Employees shown in user-facing lists (excludes built-in Dream Workflow). */
export function isUserVisibleEmployee(
  entry: Pick<RegistryEntry, "id" | "kind">,
): boolean {
  return !isDreamWorkflowEmployee(entry)
}

export function filterUserVisibleEmployees(
  employees: RegistryEntry[],
): RegistryEntry[] {
  return employees.filter(isUserVisibleEmployee)
}

/** Pick the employee whose dream.yaml schedule should be edited. */
export function pickDreamConfigTargetEmployee(
  employees: RegistryEntry[],
  preferredEmployeeId: string | null,
): string | null {
  if (preferredEmployeeId) {
    const preferred = employees.find((entry) => entry.id === preferredEmployeeId)
    if (preferred && isOrdinaryEmployee(preferred) && preferred.status === "active") {
      return preferred.id
    }
  }

  const firstOrdinary = employees.find(
    (entry) => isOrdinaryEmployee(entry) && entry.status === "active",
  )
  return firstOrdinary?.id ?? null
}
