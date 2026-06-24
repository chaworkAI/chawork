import { useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react"

import type { SessionMeta } from "@/types/session"
import { useUiLabel } from "@/hooks/useUiLabel"

export interface SessionListProps {
  sessions: SessionMeta[]
  activeSessionId: string | null
  onSessionSelect?: (id: string) => void
  onNewSession?: () => void
  onRenameSession?: (id: string, title: string) => void | Promise<void>
  onDeleteSession?: (id: string) => void | Promise<void>
}

export function SessionList({
  sessions,
  activeSessionId,
  onSessionSelect,
  onNewSession,
  onRenameSession,
  onDeleteSession,
}: SessionListProps) {
  const getLabel = useUiLabel()
  const [menuSessionId, setMenuSessionId] = useState<string | null>(null)
  const [renameTarget, setRenameTarget] = useState<SessionMeta | null>(null)
  const [renameDraft, setRenameDraft] = useState("")
  const [deleteTarget, setDeleteTarget] = useState<SessionMeta | null>(null)

  const openRename = (session: SessionMeta) => {
    setMenuSessionId(null)
    setRenameTarget(session)
    setRenameDraft(session.title)
  }

  const submitRename = () => {
    if (!renameTarget || !onRenameSession) return
    const title = renameDraft.trim()
    if (!title) return
    void onRenameSession(renameTarget.id, title)
    setRenameTarget(null)
    setRenameDraft("")
  }

  const confirmDelete = () => {
    if (!deleteTarget || !onDeleteSession) return
    void onDeleteSession(deleteTarget.id)
    setDeleteTarget(null)
    setMenuSessionId(null)
  }

  return (
    <div
      className="mt-5 flex min-h-0 flex-1 flex-col gap-2"
      data-tour-id="session-list"
    >
      <div className="flex items-center justify-between gap-2 px-1">
        <span className="text-[13px] font-bold text-muted-foreground">
          {getLabel("session.section", "会话")}
        </span>
        <button
          type="button"
          title={getLabel("session.new_title", "新建会话")}
          onClick={onNewSession}
          className="grid size-[30px] shrink-0 place-items-center rounded-full border border-line bg-white text-[18px] font-bold leading-none text-muted-foreground transition-colors hover:bg-[#f6f7f9] hover:text-ink"
        >
          ＋
        </button>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-auto pr-0.5">
        {sessions.length === 0 ? (
          <p className="px-2.5 py-2 text-[12px] text-muted-foreground">
            {getLabel("session.empty", "暂无会话")}
          </p>
        ) : null}
        {sessions.map((session) => {
          const active = session.id === activeSessionId
          const menuOpen = menuSessionId === session.id
          return (
            <div
              key={session.id}
              className={`group flex min-h-[38px] items-center gap-0.5 rounded-[14px] pr-0.5 transition-colors ${
                active
                  ? "bg-[#f4f1ed] shadow-[inset_3px_0_0_#a9a7a2]"
                  : "hover:bg-[#f6f7f9]"
              }`}
            >
              <button
                type="button"
                onClick={
                  onSessionSelect ? () => onSessionSelect(session.id) : undefined
                }
                className={`min-w-0 flex-1 truncate rounded-[13px] px-3.5 py-2 text-left text-[13px] font-semibold ${
                  active ? "text-ink" : "text-[#5a6472] group-hover:text-ink"
                }`}
                title={session.title}
              >
                {session.title}
              </button>
              <div className="relative shrink-0">
                <button
                  type="button"
                  aria-label={getLabel("session.actions", "会话操作")}
                  onClick={(e) => {
                    e.stopPropagation()
                    setMenuSessionId(menuOpen ? null : session.id)
                  }}
                  className={`grid size-7 place-items-center rounded-[10px] text-muted-foreground transition-colors hover:bg-[#f8f9fb] hover:text-ink ${
                    menuOpen ? "opacity-100" : "opacity-0 group-hover:opacity-100"
                  }`}
                >
                  <MoreHorizontal className="size-4" strokeWidth={1.75} />
                </button>
                {menuOpen ? (
                  <>
                    <button
                      type="button"
                      aria-label={getLabel("session.close_menu", "关闭菜单")}
                      className="fixed inset-0 z-40 cursor-default"
                      onClick={() => setMenuSessionId(null)}
                    />
                    <div className="absolute right-0 top-full z-50 mt-1 min-w-[120px] overflow-hidden rounded-[12px] border border-line bg-white py-1 shadow-[0_18px_42px_rgba(34,41,54,0.12)]">
                      <button
                        type="button"
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-[13px] text-ink hover:bg-[#f8f9fb]"
                        onClick={() => openRename(session)}
                      >
                        <Pencil className="size-3.5 shrink-0" strokeWidth={1.75} />
                        {getLabel("session.rename", "重命名")}
                      </button>
                      <button
                        type="button"
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-danger hover:bg-[#f8f9fb]"
                        onClick={() => {
                          setMenuSessionId(null)
                          setDeleteTarget(session)
                        }}
                      >
                        <Trash2 className="size-3.5 shrink-0" strokeWidth={1.75} />
                        {getLabel("session.delete", "删除")}
                      </button>
                    </div>
                  </>
                ) : null}
              </div>
            </div>
          )
        })}
      </div>

      <Dialog.Root
        open={renameTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setRenameTarget(null)
            setRenameDraft("")
          }
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-81 w-[min(420px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 rounded-[18px] border border-line bg-white p-[22px] text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
            <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
              会话
            </p>
            <Dialog.Title className="mt-[3px] text-[21px] font-extrabold text-ink">
              {getLabel("session.rename_title", "重命名会话")}
            </Dialog.Title>
            <Dialog.Description className="sr-only">
              输入新的会话标题
            </Dialog.Description>
            <input
              type="text"
              value={renameDraft}
              maxLength={80}
              onChange={(e) => setRenameDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) submitRename()
              }}
              className="mt-4 w-full min-h-[42px] rounded-[12px] border border-line bg-white px-3 text-[14px] text-ink outline-none focus:ring-2 focus:ring-ring/35"
              autoFocus
            />
            <div className="mt-4 flex justify-end gap-2">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="h-[36px] rounded-[12px] px-4 text-[13px] text-muted-foreground hover:bg-[#f8f9fb]"
                >
                  {getLabel("session.rename_cancel", "取消")}
                </button>
              </Dialog.Close>
              <button
                type="button"
                disabled={!renameDraft.trim()}
                onClick={submitRename}
                className="h-[36px] rounded-[12px] bg-primary px-4 text-[13px] font-bold text-primary-foreground disabled:opacity-40"
              >
                {getLabel("session.rename_save", "保存")}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-80 bg-[rgba(36,40,50,0.28)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-81 w-[min(420px,calc(100vw-80px))] -translate-x-1/2 -translate-y-1/2 rounded-[18px] border border-line bg-white p-[22px] text-ink shadow-[0_24px_70px_rgba(36,40,50,0.20)] outline-none">
            <p className="m-0 text-[12px] font-extrabold uppercase text-[var(--subtle)]">
              会话
            </p>
            <Dialog.Title className="mt-[3px] text-[21px] font-extrabold text-ink">
              {getLabel("session.delete_title", "删除会话")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-[13px] leading-relaxed text-muted-foreground">
              {getLabel(
                "session.delete_confirm",
                "确定删除「{{title}}」？此操作不可恢复。",
              ).replace("{{title}}", deleteTarget?.title ?? "")}
            </Dialog.Description>
            <div className="mt-4 flex justify-end gap-2">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="h-[36px] rounded-[12px] px-4 text-[13px] text-muted-foreground hover:bg-[#f8f9fb]"
                >
                  {getLabel("session.delete_cancel", "取消")}
                </button>
              </Dialog.Close>
              <button
                type="button"
                onClick={confirmDelete}
                className="h-[36px] rounded-[12px] border border-danger/25 bg-danger/10 px-4 text-[13px] font-bold text-danger"
              >
                {getLabel("session.delete_confirm_btn", "删除")}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  )
}
