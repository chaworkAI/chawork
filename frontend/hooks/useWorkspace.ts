import { useEffect } from "react"
import { useDomainStore } from "@/stores/domain"
import { useEmployeeStore } from "@/stores/employee"
import { useWorkspaceStore } from "@/stores/workspace"

export function useWorkspace() {
  const loadWorkspaces = useWorkspaceStore((s) => s.loadWorkspaces)

  useEffect(() => {
    void (async () => {
      await loadWorkspaces()
      await useEmployeeStore.getState().initEmployees()
      void useEmployeeStore.getState().refreshPendingReviewBadges()
      void useDomainStore.getState().loadDomainPack()
    })()
  }, [loadWorkspaces])
}
