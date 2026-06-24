export interface ComposerKeyboardInput {
  key: string
  shiftKey: boolean
  sendBlocked: boolean
  nativeEvent: {
    isComposing?: boolean
    keyCode?: number
  }
}

export function shouldSendComposerMessage(event: ComposerKeyboardInput): boolean {
  if (event.key !== "Enter") return false
  if (event.nativeEvent.isComposing) return false
  if (event.nativeEvent.keyCode === 229) return false
  if (event.shiftKey) return false
  if (event.sendBlocked) return false
  return true
}
