export interface RuntimeOwnerContext {
  activeWorkspaceId: string | null
  activeSessionId: string | null
}

export interface ResolvedRuntimeOwner {
  workspaceId: string | null
  sessionId: string | null
  legacyOwner: boolean
}

interface OwnerLike {
  workspace_id?: string
  session_id?: string
}

export function resolveRuntimeOwner(
  payload: OwnerLike,
  context: RuntimeOwnerContext,
): ResolvedRuntimeOwner {
  const hasWorkspaceOwner = Boolean(payload.workspace_id)
  return {
    workspaceId: payload.workspace_id ?? context.activeWorkspaceId,
    sessionId: payload.session_id ?? (hasWorkspaceOwner ? null : context.activeSessionId),
    legacyOwner: !hasWorkspaceOwner,
  }
}

export function eventMatchesActiveView(
  owner: ResolvedRuntimeOwner,
  context: RuntimeOwnerContext,
): boolean {
  if (!owner.workspaceId || owner.workspaceId !== context.activeWorkspaceId) {
    return false
  }
  if (!owner.sessionId) return true
  return owner.sessionId === context.activeSessionId
}
