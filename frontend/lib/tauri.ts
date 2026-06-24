import { invoke } from "@tauri-apps/api/core"

import type { DomainPack } from "@/types/domain"
import type {
  ApplyResult,
  BindingValidation,
  CreateEmployeeInput,
  DreamConfig,
  DreamDefaults,
  DreamLogEntry,
  EmployeeDetail,
  EmployeeSkillSummary,
  IntegrityReport,
  PendingUpdateRequest,
  RecentDreamResult,
  RegistryEntry,
  SkillRegistry,
  UpdateEmployeeInput,
  WorkspaceMembership,
} from "@/types/employee"
import type { Proposal, ProposalStatus, ProposalType } from "@/types/events"
import type { ImportRecord, ImportTask } from "@/types/import"
import type {
  GithubBulkDownloadResult,
  GithubDirectImportResult,
  GithubSkillPreview,
  HubEmployeeDetail,
  HubEmployeeInstallResult,
  HubEmployeeView,
  HubCompleteGithubImportOptions,
  HubGithubImportCompleteResult,
  HubGithubImportJob,
  HubListEmployeesParams,
  HubListSkillsParams,
  HubManifest,
  HubSkillDetail,
  HubSkillInstallResult,
  HubSkillView,
  PaginatedResponse,
  ProfessionInfo,
} from "@/types/hub"
import type { QmdSearchResult, QmdStatus } from "@/types/knowledge"
import type { AppLocale } from "@/types/locale"
import type { SessionMeta } from "@/types/session"
import type {
  ProviderConfigInput,
  ProviderConfigView,
  ProviderModelListResult,
} from "@/types/provider"
import type { WorkspaceState } from "@/types/workspace"
import type { Attachment } from "@/types/message"

export interface SwitchWorkspaceResult {
  workspace: WorkspaceState
  sessions: SessionMeta[]
  needs_skill_setup: boolean
}

export interface SwitchSessionResult {
  transcript: unknown[]
}

export async function listWorkspaces(): Promise<WorkspaceState[]> {
  return invoke<WorkspaceState[]>("list_workspaces")
}

export async function createWorkspace(
  name: string,
  path: string,
): Promise<WorkspaceState> {
  return invoke<WorkspaceState>("create_workspace", { name, path })
}

export async function switchWorkspace(
  path: string,
): Promise<SwitchWorkspaceResult> {
  return invoke<SwitchWorkspaceResult>("switch_workspace", { path })
}

export async function openWorkspaceDialog(
  activate = true,
): Promise<SwitchWorkspaceResult> {
  return invoke<SwitchWorkspaceResult>("open_workspace_dialog", { activate })
}

export async function listSessions(): Promise<SessionMeta[]> {
  return invoke<SessionMeta[]>("list_sessions")
}

export async function createSession(): Promise<SessionMeta> {
  return invoke<SessionMeta>("create_session")
}

export async function switchSession(
  sessionId: string,
): Promise<SwitchSessionResult> {
  return invoke<SwitchSessionResult>("switch_session", { sessionId })
}

export async function renameSession(
  sessionId: string,
  title: string,
): Promise<SessionMeta> {
  return invoke<SessionMeta>("rename_session", { sessionId, title })
}

export interface DeleteSessionResult {
  sessions: SessionMeta[]
  active_session_id: string
  transcript: unknown[]
}

export async function deleteSession(
  sessionId: string,
): Promise<DeleteSessionResult> {
  return invoke<DeleteSessionResult>("delete_session", { sessionId })
}

export async function getActiveSessionTranscript(): Promise<unknown[]> {
  return invoke<unknown[]>("get_active_session_transcript")
}

/** Starts a Codex turn in the background; progress arrives via `codex-event`. */
export async function sendCodexMessage(
  content: string,
  attachments: Attachment[],
  sessionId: string,
): Promise<void> {
  return invoke<void>("send_chat_message", {
    input: {
      content,
      sessionId,
      attachments: attachments.map((attachment) => ({
        kind: attachment.kind,
        path: attachment.path,
        dataUrl: attachment.data_url,
        name: attachment.name,
        mimeType: attachment.mime_type,
      })),
    },
  })
}

/** Reveal root workspace `runtime/provider.json` (global model config). */
export async function revealGlobalProviderConfig(): Promise<void> {
  return invoke<void>("reveal_global_provider_config")
}

export interface UiLocalePayload {
  locale: AppLocale
}

export async function getUiLocale(): Promise<UiLocalePayload> {
  return invoke<UiLocalePayload>("get_ui_locale")
}

export async function setUiLocale(locale: AppLocale): Promise<UiLocalePayload> {
  return invoke<UiLocalePayload>("set_ui_locale", { locale })
}

export interface UiPreferencesPayload {
  onboarding_tour_completed: boolean
}

export async function getUiPreferences(): Promise<UiPreferencesPayload> {
  return invoke<UiPreferencesPayload>("get_ui_preferences")
}

export async function setUiPreferences(
  preferences: UiPreferencesPayload,
): Promise<UiPreferencesPayload> {
  return invoke<UiPreferencesPayload>("set_ui_preferences", { preferences })
}

export async function getDomainPack(): Promise<DomainPack | null> {
  return invoke<DomainPack | null>("get_domain_pack")
}

export async function startWorkspaceRuntime(workspaceId?: string): Promise<void> {
  return invoke<void>("start_workspace_runtime", {
    workspaceId: workspaceId ?? null,
  })
}

export async function getRuntimeStatus(): Promise<string> {
  return invoke<string>("get_runtime_status")
}

export interface RuntimeMetadataPayload {
  runtime_status: string
  thread_id: string | null
  metadata: {
    releaseUnitId?: string
    runtimeVersion?: string
    codexVersion?: string
    capabilityMatrixVersion?: string
    capabilities?: Record<string, unknown>
    unsupportedCapabilities?: string[]
    dream?: unknown
  } | null
}

export async function getRuntimeMetadata(): Promise<RuntimeMetadataPayload> {
  return invoke<RuntimeMetadataPayload>("get_runtime_metadata")
}

export interface RefreshRuntimeContextResult {
  ok: boolean
  dirty: boolean
  restart_required: boolean
  runtime_status: string
  can_restart_now: boolean
  message?: string
}

export type RuntimeInvalidationReason =
  | "provider_changed"
  | "employee_prompt_changed"
  | "employee_skills_changed"
  | "dream_prompt_applied"
  | "workspace_binding_changed"
  | "workspace_codex_home_context_changed"
  | "mcp_context_changed"

export type RuntimeInvalidationScope = "global" | "employee" | "workspace"

export type RuntimeInvalidationPhase = "marked" | "completed"

export type RuntimeInvalidationMode =
  | "noop"
  | "immediate"
  | "deferred"
  | "completed"

export type RuntimeInvalidationUserMessageKey =
  | "settings_saved_no_active_turn"
  | "settings_saved_active_task_uses_previous"
  | "provider_settings_saved_active_task_uses_previous"
  | "employee_settings_saved_active_task_uses_previous"
  | "dream_prompt_applied_later_messages"
  | "workspace_binding_saved_later_messages"
  | "settings_saved_cleanup_warning"

export interface RuntimeInvalidationAffectedWorkspace {
  workspaceId: string
  workspacePath: string
  previousStatus: string
  mode: RuntimeInvalidationMode
}

export interface RuntimeInvalidationResult {
  ok: boolean
  invalidationId: string
  phase: RuntimeInvalidationPhase
  reason: RuntimeInvalidationReason
  scope: RuntimeInvalidationScope
  scopeIdentity: string
  userMessageKey: RuntimeInvalidationUserMessageKey | null
  invalidatedNowCount: number
  deferredCount: number
  affectedWorkspaces: RuntimeInvalidationAffectedWorkspace[]
  terminationWarnings: string[]
  message: string | null
  error: string | null
}

export interface MutationResult<T> {
  ok: boolean
  payload: T
}

export interface MutationWithRuntimeInvalidation<T> {
  mutation: MutationResult<T>
  runtimeInvalidation: RuntimeInvalidationResult
}

export async function refreshRuntimeContext(
  workspaceId: string,
  sessionId?: string,
): Promise<RefreshRuntimeContextResult> {
  return invoke<RefreshRuntimeContextResult>("refresh_runtime_context", {
    workspaceId,
    sessionId: sessionId ?? null,
  })
}

export interface RuntimeOwnerArgs {
  workspaceId: string
  sessionId: string | null
}

export async function cancelCurrentTurn(workspaceId: string): Promise<void> {
  return invoke<void>("cancel_current_turn", { workspaceId })
}

export async function respondRuntimeApproval(
  owner: RuntimeOwnerArgs,
  approvalId: string,
  decision: "accept" | "acceptForSession" | "decline" | "cancel",
): Promise<void> {
  return invoke<void>("respond_runtime_approval", {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
    approvalId,
    decision,
  })
}

export async function respondRuntimeUserInput(
  owner: RuntimeOwnerArgs,
  requestId: string,
  answers: Record<string, { answers: string[] }>,
): Promise<void> {
  return invoke<void>("respond_runtime_user_input", {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
    requestId,
    answers,
  })
}

export async function respondRuntimePermissions(
  owner: RuntimeOwnerArgs,
  requestId: string,
  granted: boolean,
  permissions: unknown,
  scope: "turn" | "session",
  strictAutoReview?: boolean,
): Promise<void> {
  return invoke<void>("respond_runtime_permissions", {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
    requestId,
    granted,
    permissions,
    scope,
    strictAutoReview,
  })
}

export async function respondRuntimeMcpElicitation(
  owner: RuntimeOwnerArgs,
  requestId: string,
  action: "accept" | "decline" | "cancel",
  content?: unknown,
  meta?: unknown,
): Promise<void> {
  return invoke<void>("respond_runtime_mcp_elicitation", {
    workspaceId: owner.workspaceId,
    sessionId: owner.sessionId,
    requestId,
    action,
    content,
    meta,
  })
}

// --- QMD Knowledge Base ---

export async function qmdInitialize(): Promise<string> {
  return invoke<string>("qmd_initialize")
}

export async function qmdRefresh(): Promise<string> {
  return invoke<string>("qmd_refresh")
}

export async function qmdStatus(): Promise<QmdStatus> {
  return invoke<QmdStatus>("qmd_status")
}

export async function qmdSearch(
  query: string,
  limit?: number,
): Promise<QmdSearchResult[]> {
  return invoke<QmdSearchResult[]>("qmd_search", { query, limit })
}

export async function qmdGetDocument(filePath: string): Promise<string> {
  return invoke<string>("qmd_get_document", { filePath })
}

export async function qmdRefreshIfStale(): Promise<boolean> {
  return invoke<boolean>("qmd_refresh_if_stale")
}

// --- Import / Wiki Knowledge Build ---

/** Submits an import; returns a task id immediately. Poll `getImportTask` for progress. */
export async function importFile(sourcePath: string): Promise<string> {
  return invoke<string>("import_file", { sourcePath })
}

export async function getImportTask(taskId: string): Promise<ImportTask> {
  return invoke<ImportTask>("get_import_task", { taskId })
}

export async function listImportTasks(limit?: number): Promise<ImportTask[]> {
  return invoke<ImportTask[]>("list_import_tasks", { limit })
}

/** Legacy log feed. Prefer `listImportTasks` for status. */
export async function listImports(limit?: number): Promise<ImportRecord[]> {
  return invoke<ImportRecord[]>("list_imports", { limit })
}

// --- Proposals / Review ---

export async function createProposal(params: {
  title: string
  description: string
  proposalType: ProposalType
  targetPath?: string
  diff?: string
  newContent?: string
  sourceSession?: string
  risk?: string
}): Promise<Proposal> {
  return invoke<Proposal>("create_proposal", {
    title: params.title,
    description: params.description,
    proposalType: params.proposalType,
    targetPath: params.targetPath,
    diff: params.diff,
    newContent: params.newContent,
    sourceSession: params.sourceSession,
    risk: params.risk,
  })
}

export async function listProposals(statusFilter?: ProposalStatus): Promise<Proposal[]> {
  return invoke<Proposal[]>("list_proposals", { statusFilter })
}

export async function getProposal(id: string): Promise<Proposal> {
  return invoke<Proposal>("get_proposal", { id })
}

export async function applyProposal(id: string): Promise<Proposal> {
  return invoke<Proposal>("apply_proposal", { id })
}

export async function rejectProposal(id: string): Promise<Proposal> {
  return invoke<Proposal>("reject_proposal", { id })
}

// --- Global settings (root workspace) ---

export interface GlobalProviderPayload {
  configured: boolean
  model: string
  instructions: string
  openai_base_url: string
  openai_api_key: string
}

export async function getGlobalProvider(): Promise<GlobalProviderPayload> {
  return invoke<GlobalProviderPayload>("get_global_provider")
}

export async function setGlobalProviderModel(
  model: string,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("set_global_provider_model", { model })
}

export async function setGlobalProviderInstructions(
  instructions: string,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>(
    "set_global_provider_instructions",
    { instructions },
  )
}

export async function setGlobalProviderConnection(payload: {
  openai_base_url: string
  openai_api_key: string
}): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("set_global_provider_connection", {
    openaiBaseUrl: payload.openai_base_url,
    openaiApiKey: payload.openai_api_key,
  })
}

export async function isGlobalProviderConfigured(): Promise<boolean> {
  return invoke<boolean>("is_global_provider_configured")
}

export interface RootWorkspaceInfoPayload {
  path: string
  codex_home: string
  provider_path: string
  skills_dir: string
  templates_dir: string
  mcp_dir: string
}

export async function getRootWorkspaceInfo(): Promise<RootWorkspaceInfoPayload> {
  return invoke<RootWorkspaceInfoPayload>("get_root_workspace_info")
}

// --- Workspace config (effective provider + tool policy) ---

export type EffectiveProviderErrorKind =
  | "no_workspace"
  | "global_not_configured"

export interface EffectiveProviderPayload {
  configured: boolean
  origin: "inherit_global" | "none"
  model: string
  error_kind: EffectiveProviderErrorKind | null
  error_message: string | null
}

export async function getEffectiveProvider(): Promise<EffectiveProviderPayload> {
  return invoke<EffectiveProviderPayload>("get_effective_provider")
}

export interface ToolPolicyPayload {
  default_action: "enabled" | "disabled"
  overrides: Record<string, "enabled" | "disabled">
}

export async function getToolPolicy(): Promise<ToolPolicyPayload> {
  return invoke<ToolPolicyPayload>("get_tool_policy")
}

export async function setToolPolicy(
  payload: ToolPolicyPayload,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("set_tool_policy", {
    defaultAction: payload.default_action,
    overrides: payload.overrides,
  })
}

// --- Provider (global-only runtime auth) ---

/** Save global provider config (single write to root `runtime/provider.json`). */
export async function setGlobalProvider(
  config: ProviderConfigInput,
): Promise<MutationWithRuntimeInvalidation<ProviderConfigView>> {
  return invoke<MutationWithRuntimeInvalidation<ProviderConfigView>>(
    "set_global_provider",
    { config },
  )
}

/** Fetch OpenAI-compatible models from `{base_url}/models` through the backend. */
export async function listProviderModels(
  config?: ProviderConfigInput,
): Promise<ProviderModelListResult> {
  return invoke<ProviderModelListResult>("list_provider_models", { config: config ?? null })
}

/** Local HTTP server port (MCP / runtime integrations). */
export async function getHttpServerPort(): Promise<number> {
  return invoke<number>("get_http_server_port")
}

// --- Skill Hub ---

export async function hubGetManifest(): Promise<HubManifest> {
  return invoke<HubManifest>("hub_get_manifest")
}

export async function hubListProfessions(): Promise<ProfessionInfo[]> {
  return invoke<ProfessionInfo[]>("hub_list_professions")
}

export async function hubListSkills(
  params: HubListSkillsParams = {},
): Promise<PaginatedResponse<HubSkillView>> {
  return invoke<PaginatedResponse<HubSkillView>>("hub_list_skills", {
    query: params.query ?? null,
    profession: params.profession ?? null,
    filter: params.filter ?? null,
    page: params.page ?? null,
    limit: params.limit ?? null,
  })
}

export async function hubGetSkillDetail(
  hubSkillId: string,
): Promise<HubSkillDetail> {
  return invoke<HubSkillDetail>("hub_get_skill_detail", { hubSkillId })
}

export async function hubInstallSkill(
  hubSkillId: string,
): Promise<HubSkillInstallResult> {
  return invoke<HubSkillInstallResult>("hub_install_skill", { hubSkillId })
}

export async function hubUninstallSkill(
  hubSkillId: string,
): Promise<{ hub_id: string; local_id: string }> {
  return invoke<{ hub_id: string; local_id: string }>("hub_uninstall_skill", {
    hubSkillId,
  })
}

export async function hubListEmployees(
  params: HubListEmployeesParams = {},
): Promise<PaginatedResponse<HubEmployeeView>> {
  return invoke<PaginatedResponse<HubEmployeeView>>("hub_list_employees", {
    query: params.query ?? null,
    tags: params.tags ?? null,
    filter: params.filter ?? null,
    page: params.page ?? null,
    limit: params.limit ?? null,
  })
}

export async function hubGetEmployeeDetail(
  hubEmployeeId: string,
): Promise<HubEmployeeDetail> {
  return invoke<HubEmployeeDetail>("hub_get_employee_detail", { hubEmployeeId })
}

export async function hubInstallEmployee(
  hubEmployeeId: string,
): Promise<HubEmployeeInstallResult> {
  return invoke<HubEmployeeInstallResult>("hub_install_employee", {
    hubEmployeeId,
  })
}

export async function hubStartGithubImport(
  url: string,
  gitRef?: string,
): Promise<HubGithubImportJob> {
  return invoke<HubGithubImportJob>("hub_start_github_import", {
    url,
    gitRef: gitRef ?? null,
  })
}

export async function hubGetGithubImportJob(
  jobId: string,
): Promise<HubGithubImportJob> {
  return invoke<HubGithubImportJob>("hub_get_github_import_job", { jobId })
}

export async function hubCompleteGithubImport(
  repoUrl: string,
  hubSkillIds: string[],
  options: HubCompleteGithubImportOptions,
): Promise<HubGithubImportCompleteResult> {
  return invoke<HubGithubImportCompleteResult>("hub_complete_github_import", {
    repoUrl,
    hubSkillIds,
    createEmployee: options.createEmployee === true,
    employeePrompt: options.employeePrompt?.trim() || null,
  })
}

// --- GitHub Direct Scan (不依赖 Hub API) ---

export async function githubScanRepo(
  url: string,
  gitRef?: string,
): Promise<GithubSkillPreview[]> {
  return invoke<GithubSkillPreview[]>("github_scan_repo", {
    url,
    gitRef: gitRef ?? null,
  })
}

export async function githubDownloadAllSkills(
  url: string,
  skillPaths: string[],
  gitRef?: string,
): Promise<GithubBulkDownloadResult> {
  return invoke<GithubBulkDownloadResult>("github_download_all_skills", {
    url,
    skillPaths,
    gitRef: gitRef ?? null,
  })
}

export async function githubCompleteImport(
  url: string,
  skillPaths: string[],
  gitRef?: string,
  syncAsEmployee?: boolean,
  employeePrompt?: string,
): Promise<GithubDirectImportResult> {
  return invoke<GithubDirectImportResult>("github_complete_import", {
    url,
    skillPaths,
    gitRef: gitRef ?? null,
    syncAsEmployee: syncAsEmployee ?? false,
    employeePrompt: employeePrompt?.trim() || null,
  })
}

// --- Employee ---

export async function listEmployees(): Promise<RegistryEntry[]> {
  return invoke<RegistryEntry[]>("list_employees")
}

export async function getEmployeeDetail(id: string): Promise<EmployeeDetail> {
  return invoke<EmployeeDetail>("get_employee_detail", { id })
}

export async function createEmployee(
  input: CreateEmployeeInput,
): Promise<EmployeeDetail> {
  return invoke<EmployeeDetail>("create_employee", { input })
}

export async function updateEmployeeMetadata(
  id: string,
  input: UpdateEmployeeInput,
): Promise<EmployeeDetail> {
  return invoke<EmployeeDetail>("update_employee_metadata", { id, input })
}

export async function deleteEmployee(
  id: string,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("delete_employee", { id })
}

export async function checkEmployeeIntegrity(
  id: string,
): Promise<IntegrityReport> {
  return invoke<IntegrityReport>("check_employee_integrity", { id })
}

export async function readEmployeePrompt(
  employeeId: string,
): Promise<string> {
  return invoke<string>("read_employee_prompt", { employeeId })
}

export async function writeEmployeePrompt(
  employeeId: string,
  content: string,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("write_employee_prompt", { employeeId, content })
}

export async function listEmployeeSkills(
  employeeId: string,
): Promise<EmployeeSkillSummary[]> {
  return invoke<EmployeeSkillSummary[]>("list_employee_skills", { employeeId })
}

export async function copyRootSkillToEmployee(
  employeeId: string,
  skillId: string,
): Promise<MutationWithRuntimeInvalidation<EmployeeSkillSummary>> {
  return invoke<MutationWithRuntimeInvalidation<EmployeeSkillSummary>>("copy_root_skill_to_employee", {
    employeeId,
    skillId,
  })
}

export async function toggleEmployeeSkill(
  employeeId: string,
  skillId: string,
  enabled: boolean,
): Promise<MutationWithRuntimeInvalidation<SkillRegistry>> {
  return invoke<MutationWithRuntimeInvalidation<SkillRegistry>>("toggle_employee_skill", {
    employeeId,
    skillId,
    enabled,
  })
}

export async function deleteEmployeeSkill(
  employeeId: string,
  skillId: string,
): Promise<MutationWithRuntimeInvalidation<SkillRegistry>> {
  return invoke<MutationWithRuntimeInvalidation<SkillRegistry>>("delete_employee_skill", {
    employeeId,
    skillId,
  })
}

// --- Workspace Binding ---

export async function bindWorkspaceToEmployee(
  employeeId: string,
  workspacePath?: string,
): Promise<MutationWithRuntimeInvalidation<BindingValidation>> {
  return invoke<MutationWithRuntimeInvalidation<BindingValidation>>("bind_workspace_to_employee", {
    employeeId,
    workspacePath: workspacePath ?? null,
  })
}

export async function unbindWorkspaceFromEmployee(
  workspacePath?: string,
): Promise<MutationWithRuntimeInvalidation<void>> {
  return invoke<MutationWithRuntimeInvalidation<void>>("unbind_workspace_from_employee", {
    workspacePath: workspacePath ?? null,
  })
}

export async function validateWorkspaceBinding(
  workspacePath?: string,
): Promise<BindingValidation> {
  return invoke<BindingValidation>("validate_workspace_binding", {
    workspacePath: workspacePath ?? null,
  })
}

export async function listWorkspacesForEmployee(
  employeeId: string,
): Promise<WorkspaceMembership[]> {
  return invoke<WorkspaceMembership[]>("list_workspaces_for_employee", {
    employeeId,
  })
}

// --- Dream ---

export async function getDreamLog(
  limit: number,
): Promise<DreamLogEntry[]> {
  return invoke<DreamLogEntry[]>("get_dream_log", { limit })
}

export async function getDreamConfig(
  employeeId: string,
): Promise<DreamConfig> {
  return invoke<DreamConfig>("get_dream_config", { employeeId })
}

export async function setDreamConfig(
  employeeId: string,
  config: DreamConfig,
): Promise<void> {
  return invoke<void>("set_dream_config", { employeeId, config })
}

export async function getRecentDreamResult(
  employeeId: string,
): Promise<RecentDreamResult | null> {
  return invoke<RecentDreamResult | null>("get_recent_dream_result", {
    employeeId,
  })
}

export async function getPendingRequest(
  employeeId: string,
): Promise<PendingUpdateRequest | null> {
  return invoke<PendingUpdateRequest | null>("get_pending_request", {
    employeeId,
  })
}

export async function listEmployeesWithPendingDreamRequests(): Promise<string[]> {
  return invoke<string[]>("list_employees_with_pending_dream_requests")
}

export async function rejectDreamRequest(
  employeeId: string,
): Promise<void> {
  return invoke<void>("reject_dream_request", { employeeId })
}

export async function approveDreamRequest(
  employeeId: string,
): Promise<MutationWithRuntimeInvalidation<ApplyResult>> {
  return invoke<MutationWithRuntimeInvalidation<ApplyResult>>("approve_dream_request", { employeeId })
}

export async function getDreamDefaults(): Promise<DreamDefaults> {
  return invoke<DreamDefaults>("get_dream_defaults")
}

export async function setDreamDefaults(
  defaults: DreamDefaults,
): Promise<void> {
  return invoke<void>("set_dream_defaults", { defaults })
}

export async function runDreamPhase1(
  employeeId: string,
): Promise<RecentDreamResult> {
  return invoke<RecentDreamResult>("run_dream_phase1", { employeeId })
}

export async function cancelDreamRun(): Promise<void> {
  return invoke<void>("cancel_dream_run")
}
