import { Loader2, CheckCircle2, XCircle, AlertTriangle } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useUiLabel } from "@/hooks/useUiLabel"
import { applyLabelTemplate } from "@/lib/builtinLabels"
import { useEmployeeStore } from "@/stores/employee"

function reviewActionLabel(
  label: ReturnType<typeof useUiLabel>,
  action: string,
): string {
  switch (action.toLowerCase()) {
    case "add":
      return label("employee.review.action.add", "新增")
    case "modify":
      return label("employee.review.action.modify", "修改")
    case "remove":
      return label("employee.review.action.remove", "删除")
    default:
      return action
  }
}

function reviewStatusLabel(
  label: ReturnType<typeof useUiLabel>,
  status: string,
): string {
  switch (status.toLowerCase()) {
    case "pending":
      return label("employee.review.status.pending", "待审批")
    case "approved":
      return label("employee.review.status.approved", "已批准")
    case "applying":
      return label("employee.review.status.applying", "应用中")
    case "applied":
      return label("employee.review.status.applied", "已应用")
    case "rejected":
      return label("employee.review.status.rejected", "已拒绝")
    case "failed":
      return label("employee.review.status.failed", "失败")
    default:
      return status
  }
}

export function EmployeeReviewQueue() {
  const label = useUiLabel()
  const selectedDetail = useEmployeeStore((s) => s.selectedDetail)
  const employeeId = selectedDetail?.registry_entry.id
  const pendingRequest = useEmployeeStore((s) =>
    employeeId && s.dreamStateEmployeeId === employeeId ? s.pendingRequest : null,
  )
  const isApplying = useEmployeeStore(
    (s) => employeeId != null && s.applyingEmployeeId === employeeId,
  )
  const isDreamStateLoading = useEmployeeStore(
    (s) =>
      employeeId != null &&
      s.detailLoadingEmployeeId === employeeId &&
      s.dreamStateEmployeeId !== employeeId,
  )
  const applyResult = useEmployeeStore((s) => s.applyResult)
  const approveRequest = useEmployeeStore((s) => s.approveRequest)
  const rejectRequest = useEmployeeStore((s) => s.rejectRequest)
  const clearApplyResult = useEmployeeStore((s) => s.clearApplyResult)

  if (!employeeId) return null

  if (isDreamStateLoading) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-16">
        <Loader2 className="size-6 animate-spin text-primary" />
        <p className="text-[13px] text-muted-foreground">
          {label("employee.review.loading", "加载审批队列...")}
        </p>
      </div>
    )
  }

  if (isApplying || pendingRequest?.result.status === "applying") {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-16">
        <Loader2 className="size-6 animate-spin text-primary" />
        <p className="text-[13px] text-muted-foreground">
          {label("employee.review.applying", "正在应用 prompt 更新...")}
        </p>
      </div>
    )
  }

  if (applyResult) {
    return (
      <div className="grid gap-4">
        <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <div className="mb-3 flex items-center gap-2">
            {applyResult.success ? (
              <CheckCircle2 className="size-5 text-emerald-600" />
            ) : (
              <XCircle className="size-5 text-danger" />
            )}
            <span className="text-[14px] font-bold">
              {applyResult.success
                ? label("employee.review.success", "Prompt 更新成功")
                : label("employee.review.failed", "Prompt 更新失败")}
            </span>
          </div>
          {applyResult.error && (
            <p className="mb-3 text-[12px] text-danger">{applyResult.error}</p>
          )}
          <Button type="button" size="sm" className="h-[36px] rounded-[12px] px-4" onClick={clearApplyResult}>
            {label("employee.review.confirm_ok", "确定")}
          </Button>
        </section>
      </div>
    )
  }

  if (!pendingRequest) {
    return (
      <p className="py-12 text-center text-[13px] text-muted-foreground">
        {label("employee.review.empty", "暂无待审批的 prompt 更新请求")}
      </p>
    )
  }

  const { result } = pendingRequest
  const reviewStatus = result.status?.toLowerCase() ?? "pending"
  const canReject = reviewStatus === "pending"
  const canApprove =
    reviewStatus === "pending" ||
    reviewStatus === "approved" ||
    reviewStatus === "failed"
  const reviewTitle =
    reviewStatus === "failed"
      ? label("employee.review.failed_title", "Prompt 更新失败")
      : label("employee.review.pending_title", "待审批更新")

  return (
    <div className="grid gap-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <AlertTriangle className="size-4 text-amber-600" />
            <h3 className="text-[14px] font-bold">{reviewTitle}</h3>
          </div>
          <p className="text-[11px] text-muted-foreground">
            {applyLabelTemplate(
              label(
                "employee.review.dream_run_meta",
                "Dream 运行: {{runId}} · {{createdAt}}",
              ),
              {
                runId: pendingRequest.dream_run_id,
                createdAt: pendingRequest.created_at,
              },
            )}
            {result.status && (
              <span className="ml-2 rounded-[10px] border border-[#e4d4ac] bg-[#fff8e7] px-2 py-1 text-[10px] font-bold text-[#6f5b34]">
                {reviewStatusLabel(label, result.status)}
              </span>
            )}
          </p>
          {result.source_prompt_path && (
            <p className="text-[10px] text-muted-foreground font-mono mt-0.5">
              {applyLabelTemplate(
                label("employee.review.target_path", "目标: {{path}}"),
                { path: result.source_prompt_path },
              )}
            </p>
          )}
        </div>
      </div>

      {reviewStatus === "failed" && pendingRequest.error_message ? (
        <section className="rounded-[15px] border border-danger/30 bg-danger/5 p-3.5">
          <p className="text-[12px] text-danger/90">{pendingRequest.error_message}</p>
        </section>
      ) : null}

      <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <h4 className="mb-2 text-[14px] font-bold text-ink">
          {label("employee.review.summary", "摘要")}
        </h4>
        <p className="text-[13px] text-ink">{result.summary}</p>
        {result.impact && (
          <p className="mt-2 text-[12px] text-muted-foreground">
            {applyLabelTemplate(
              label("employee.review.impact", "影响范围: {{impact}}"),
              { impact: result.impact },
            )}
          </p>
        )}
      </section>

      <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
        <h4 className="mb-2 text-[14px] font-bold text-ink">
          {label("employee.review.source_sessions", "基于会话")}
        </h4>
        <div className="space-y-1">
          {result.source_sessions.map((s, i) => (
            <div key={i} className="text-[11px] text-muted-foreground font-mono">
              {s.workspace_id} / {s.session_id}
              {s.last_updated_at && (
                <span className="ml-2 text-[10px] opacity-70">({s.last_updated_at})</span>
              )}
            </div>
          ))}
        </div>
      </section>

      {result.updates && result.updates.length > 0 && (
        <section className="rounded-[15px] border border-line-soft bg-[#f8f9fb] p-3.5">
          <h4 className="mb-2 text-[14px] font-bold text-ink">
            {applyLabelTemplate(
              label("employee.review.updates_heading", "更新内容 ({{count}} 条)"),
              { count: String(result.updates.length) },
            )}
          </h4>
          <div className="space-y-3">
            {result.updates.map((update, i) => (
              <div
                key={i}
                className="rounded-[13px] border border-line-soft bg-white p-3"
              >
                <div className="mb-1 flex items-center gap-2">
                    <span className="rounded-[10px] border border-line bg-[#eef1f5] px-2 py-1 text-[10px] font-bold text-primary">
                      {reviewActionLabel(label, update.action)}
                    </span>
                  {update.section && (
                    <span className="text-[12px] font-medium text-ink">
                      {update.section}
                    </span>
                  )}
                </div>
                <pre className="mb-2 whitespace-pre-wrap rounded-[13px] border border-line-soft bg-[#f8f9fb] p-3 font-mono text-[11px] text-ink">
                  {update.content}
                </pre>
                <p className="text-[11px] text-muted-foreground">
                  {applyLabelTemplate(
                    label("employee.review.reason", "原因: {{reason}}"),
                    { reason: update.reason },
                  )}
                </p>
              </div>
            ))}
          </div>
        </section>
      )}

      <div className="flex items-center gap-3 border-t border-line-soft pt-4">
        {canApprove ? (
          <Button
            type="button"
            size="sm"
            className="h-[36px] rounded-[12px] px-4"
            onClick={() => {
              if (
                window.confirm(
                  label(
                    "employee.review.confirm_approve",
                    "确认批准此 prompt 更新？批准后将立即应用到员工 prompt。",
                  ),
                )
              ) {
                void approveRequest(employeeId)
              }
            }}
          >
            <CheckCircle2 className="mr-1.5 size-3.5" />
            {reviewStatus === "approved"
              ? label("employee.review.retry_approve", "重试批准")
              : reviewStatus === "failed"
                ? label("employee.review.retry_failed", "重试应用")
                : label("employee.review.approve", "批准并应用")}
          </Button>
        ) : null}
        {canReject ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-[36px] rounded-[12px] bg-white px-4"
            onClick={() => {
              if (
                window.confirm(
                  label("employee.review.confirm_reject", "确认拒绝此更新请求？"),
                )
              ) {
                void rejectRequest(employeeId)
              }
            }}
          >
            <XCircle className="mr-1.5 size-3.5" />
            {label("employee.review.reject", "拒绝")}
          </Button>
        ) : null}
      </div>
    </div>
  )
}
