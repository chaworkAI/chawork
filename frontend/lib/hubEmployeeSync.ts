import { removeUserCustomGithubEmployeeId } from "@/lib/hubCustomEmployees"

/** Hub 市场与员工面板删除 GitHub/本地员工后，同步 localStorage 与 Hub store 列表。 */
export async function syncGithubEmployeeRemoved(employeeId: string) {
  const userCustomGithubEmployeeIds = removeUserCustomGithubEmployeeId(employeeId)
  const { useHubStore } = await import("@/stores/hub")
  useHubStore.setState((state) => ({
    userCustomGithubEmployeeIds,
    employees: state.employees.filter((employee) => employee.id !== employeeId),
  }))
  return userCustomGithubEmployeeIds
}
