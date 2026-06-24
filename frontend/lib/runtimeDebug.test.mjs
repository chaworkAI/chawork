import assert from "node:assert/strict"
import { access, readFile } from "node:fs/promises"
import test from "node:test"

async function readProjectFile(path) {
  return readFile(new URL(`../../${path}`, import.meta.url), "utf8")
}

async function projectFileExists(path) {
  try {
    await access(new URL(`../../${path}`, import.meta.url))
    return true
  } catch {
    return false
  }
}

test("product chat does not use the removed HTTP stream stub", async () => {
  const [chatStore, httpHandler] = await Promise.all([
    readProjectFile("frontend/stores/chat.ts"),
    readProjectFile("backend/src/http_server/handlers/chat_stream.rs"),
  ])

  assert.doesNotMatch(chatStore, /sendMessageViaHttp/)
  assert.doesNotMatch(chatStore, /createStreamUrl|streamNdjson|\/api\/chat\/stream/)
  assert.equal(await projectFileExists("frontend/hooks/useHttpStream.ts"), false)
  assert.equal(await projectFileExists("frontend/lib/httpStream.ts"), false)

  assert.match(httpHandler, /StatusCode::GONE/)
  assert.match(httpHandler, /http_chat_stream_removed/)
  assert.doesNotMatch(httpHandler, /Received:|Processing your request|assistant_delta/)
})

test("legacy direct OpenAI stream path and prepend-instructions wording are removed", async () => {
  const [servicesMod, zhLabels, enLabels] = await Promise.all([
    readProjectFile("backend/src/services/mod.rs"),
    readProjectFile("frontend/lib/builtinLabels.ts"),
    readProjectFile("frontend/lib/localeLabels/en-US.ts"),
  ])

  assert.equal(await projectFileExists("backend/src/services/openai_compat_stream.rs"), false)
  assert.equal(await projectFileExists("src-tauri/src/services/openai_compat_stream.rs"), false)
  assert.doesNotMatch(servicesMod, /openai_compat_stream/)
  assert.doesNotMatch(zhLabels, /自动前置/)
  assert.doesNotMatch(enLabels, /prepended/)
  assert.match(zhLabels, /通过 runtime 准备的 CODEX_HOME 生效/)
  assert.match(enLabels, /runtime-prepared CODEX_HOME/)
})

test("dream product path uses structured runtime results and phase2 prompt candidates", async () => {
  const [commands, dreamPhase1, dreamService] = await Promise.all([
    readProjectFile("backend/src/commands/dream.rs"),
    readProjectFile("backend/src/services/dream_phase1.rs"),
    readProjectFile("backend/src/services/dream.rs"),
  ])

  assert.match(dreamPhase1, /process_dream_result\(&app_state\.root, &result\)/)
  assert.match(commands, /\.phase2\(&app, "current", &req\.result, &target_prompt_path\)/)
  assert.match(commands, /apply_prompt_and_complete_request\(/)

  assert.doesNotMatch(dreamService, /fn parse_dream_output/)
  assert.doesNotMatch(dreamService, /fn submit_dream_output/)
  assert.doesNotMatch(dreamService, /fn handle_dream_parse_failure/)
  assert.doesNotMatch(dreamService, /fn apply_prompt_update/)
  assert.doesNotMatch(dreamService, /fn append_section|fn replace_section|fn remove_section/)
})

test("runtime debug events are visible but do not drive runtime state", async () => {
  const [types, hook, inspector, store] = await Promise.all([
    readProjectFile("frontend/types/events.ts"),
    readProjectFile("frontend/hooks/useCodexEvents.ts"),
    readProjectFile("frontend/components/runtime/RuntimeInspector.tsx"),
    readProjectFile("frontend/stores/runtime.ts"),
  ])

  assert.match(types, /type: "runtime_debug"/)
  assert.match(hook, /case "runtime_debug"/)
  assert.match(inspector, /t === "runtime_debug"/)
  assert.doesNotMatch(store, /case "runtime_debug"/)
})

test("turn complete usage keeps codex token breakdown fields", async () => {
  const mapper = await readProjectFile("frontend/lib/runtimeEventMap.ts")

  assert.match(mapper, /input_tokens: event\.usage\.input_tokens/)
  assert.match(mapper, /cached_input_tokens: event\.usage\.cached_input_tokens/)
  assert.match(mapper, /output_tokens: event\.usage\.output_tokens/)
  assert.match(mapper, /reasoning_output_tokens: event\.usage\.reasoning_output_tokens/)
  assert.match(mapper, /model_context_window: event\.usage\.model_context_window/)
})

test("plan events are projected and displayed", async () => {
  const [types, mapper, hook, card] = await Promise.all([
    readProjectFile("frontend/types/events.ts"),
    readProjectFile("frontend/lib/runtimeEventMap.ts"),
    readProjectFile("frontend/hooks/useCodexEvents.ts"),
    readProjectFile("frontend/components/runtime/EventCard.tsx"),
  ])

  assert.match(types, /type: "plan_update"/)
  assert.match(mapper, /case "plan_update"/)
  assert.match(mapper, /case "plan_delta"/)
  assert.match(mapper, /case "plan_done"/)
  assert.match(hook, /case "plan_update"/)
  assert.match(card, /case "plan_update"/)
})

test("tool and file progress deltas are projected and displayed", async () => {
  const [types, mapper, hook, card] = await Promise.all([
    readProjectFile("frontend/types/events.ts"),
    readProjectFile("frontend/lib/runtimeEventMap.ts"),
    readProjectFile("frontend/hooks/useCodexEvents.ts"),
    readProjectFile("frontend/components/runtime/EventCard.tsx"),
  ])

  assert.match(types, /type: "tool_delta"/)
  assert.match(types, /type: "file_change_delta"/)
  assert.match(mapper, /case "tool_delta"/)
  assert.match(mapper, /case "file_change_delta"/)
  assert.match(hook, /case "tool_delta"/)
  assert.match(hook, /case "file_change_delta"/)
  assert.match(card, /case "tool_delta"/)
  assert.match(card, /case "file_change_delta"/)
})

test("mcp progress and status events are projected and displayed", async () => {
  const [types, mapper, hook, card] = await Promise.all([
    readProjectFile("frontend/types/events.ts"),
    readProjectFile("frontend/lib/runtimeEventMap.ts"),
    readProjectFile("frontend/hooks/useCodexEvents.ts"),
    readProjectFile("frontend/components/runtime/EventCard.tsx"),
  ])

  assert.match(types, /type: "mcp_oauth_login_completed"/)
  assert.match(types, /type: "mcp_server_status_updated"/)
  assert.match(mapper, /case "mcp_oauth_login_completed"/)
  assert.match(mapper, /case "mcp_server_status_updated"/)
  assert.match(hook, /case "mcp_oauth_login_completed"/)
  assert.match(hook, /case "mcp_server_status_updated"/)
  assert.match(card, /case "mcp_oauth_login_completed"/)
  assert.match(card, /case "mcp_server_status_updated"/)
})

test("mcp elicitation prompts submit content through the dedicated runtime response", async () => {
  const [app, store, promptCard] = await Promise.all([
    readProjectFile("frontend/App.tsx"),
    readProjectFile("frontend/stores/runtime.ts"),
    readProjectFile("frontend/components/chat/UserPromptCard.tsx"),
  ])

  assert.match(app, /answerMcpElicitation/)
  assert.match(app, /entry\?\.mcp_elicitation/)
  assert.match(store, /answerMcpElicitation: async/)
  assert.match(store, /respondRuntimeMcpElicitation\([\s\S]*"accept"[\s\S]*content[\s\S]*meta/)
  assert.match(promptCard, /JSON\.parse\(trimmed\)/)
  assert.match(promptCard, /onPositive\(entry\.id, content\)/)

  const mcpBranches = [...store.matchAll(/else if \(entry\.mcp_elicitation\) \{([\s\S]*?)\n      \} else/g)]
  assert.ok(mcpBranches.length >= 2, "expected explicit MCP elicitation branches")
  for (const [, branch] of mcpBranches) {
    assert.match(branch, /respondRuntimeMcpElicitation/)
    assert.doesNotMatch(branch, /respondRuntimeApproval/)
  }
})

test("user input prompts support option and free-form answers", async () => {
  const promptCard = await readProjectFile("frontend/components/chat/UserPromptCard.tsx")

  assert.match(promptCard, /const \[textAnswers/)
  assert.match(promptCard, /question\.isOther/)
  assert.match(promptCard, /question\.isSecret \? "password" : "text"/)
  assert.match(promptCard, /answers: questionAnswer/)
  assert.match(promptCard, /submitUserInputAnswers/)
  assert.doesNotMatch(promptCard, /options\.length === 0 \|\| Boolean/)
})

test("runtime lifecycle notices survive successful completion and use provider settings wording", async () => {
  const [runtimeStore, toastStore, tauriTypes, backendLifecycle] = await Promise.all([
    readProjectFile("frontend/stores/runtime.ts"),
    readProjectFile("frontend/stores/toast.ts"),
    readProjectFile("frontend/lib/tauri.ts"),
    readProjectFile("backend/src/runtime/lifecycle.rs"),
  ])

  assert.match(tauriTypes, /provider_settings_saved_active_task_uses_previous/)
  assert.doesNotMatch(tauriTypes, /provider_key_saved_active_task_uses_previous/)
  assert.match(backendLifecycle, /ProviderSettingsSavedActiveTaskUsesPrevious/)
  assert.doesNotMatch(backendLifecycle, /ProviderKeySavedActiveTaskUsesPrevious/)
  assert.match(runtimeStore, /模型配置已保存。当前任务会按修改前的模型配置完成/)
  assert.doesNotMatch(runtimeStore, /密钥已保存。正在运行的任务/)

  const upsertMatch = runtimeStore.match(
    /upsertLifecycleNotice: \(payload\) => \{([\s\S]*?)\n  \},\n\n  handleRuntimeInvalidation/,
  )
  assert.ok(upsertMatch, "expected upsertLifecycleNotice implementation")
  assert.doesNotMatch(
    upsertMatch[1],
    /dismiss/,
    "successful completed lifecycle events must not immediately remove the saved-settings notice",
  )

  assert.match(toastStore, /const toastTimers = new Map<string, number>\(\)/)
  assert.match(toastStore, /window\.clearTimeout\(existingTimer\)/)
  assert.match(toastStore, /toastTimers\.set\(id, timer\)/)
})
