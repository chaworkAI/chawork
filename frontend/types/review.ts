export type ProposalStatus = "pending" | "accepted" | "rejected" | "applying" | "error"
export type SkillProposalAction = "reject" | "accept_workspace" | "accept_global"

export interface ReviewProposal {
  id: string
  title: string
  description: string
  target_path: string
  target_scope: "workspace" | "root"
  executor: "workspace-tools" | "chawork-app"
  impact_summary?: string
  risk: "low" | "medium" | "high"
  diff?: string
  status: ProposalStatus
  is_skill_proposal: boolean
  skill_id?: string
  affected_workspaces?: string[]
  created_at: string
  resolved_at?: string
}
