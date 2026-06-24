import { create } from "zustand"

export type ToastVariant = "success" | "error" | "info" | "warning"

export interface Toast {
  id: string
  message: string
  variant: ToastVariant
}

interface ToastState {
  toasts: Toast[]
  show: (message: string, variant?: ToastVariant, id?: string) => void
  dismiss: (id: string) => void
}

const toastTimers = new Map<string, number>()

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],

  show: (message, variant = "success", id = crypto.randomUUID()) => {
    const existingTimer = toastTimers.get(id)
    if (existingTimer !== undefined) {
      window.clearTimeout(existingTimer)
    }
    set((state) => ({
      toasts: state.toasts.some((toast) => toast.id === id)
        ? state.toasts.map((toast) =>
            toast.id === id ? { id, message, variant } : toast,
          )
        : [...state.toasts, { id, message, variant }],
    }))
    const timer = window.setTimeout(() => {
      toastTimers.delete(id)
      get().dismiss(id)
    }, 3000)
    toastTimers.set(id, timer)
  },

  dismiss: (id) => {
    const existingTimer = toastTimers.get(id)
    if (existingTimer !== undefined) {
      window.clearTimeout(existingTimer)
      toastTimers.delete(id)
    }
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }))
  },
}))
