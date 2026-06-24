import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"

async function readProjectFile(path) {
  return readFile(new URL(`../../${path}`, import.meta.url), "utf8")
}

test("app renders a guided onboarding tour overlay", async () => {
  const app = await readProjectFile("frontend/App.tsx")
  const tourOverlay = await readProjectFile("frontend/components/onboarding/OnboardingTourOverlay.tsx")
  const tauri = await readProjectFile("frontend/lib/tauri.ts")
  const globalSettingsCommands = await readProjectFile("backend/src/commands/global_settings.rs")
  const backendLib = await readProjectFile("backend/src/lib.rs")

  assert.match(app, /OnboardingTourOverlay[\s\S]*from "@\/components\/onboarding\/OnboardingTourOverlay"/)
  assert.doesNotMatch(app, /OnboardingCoachMark/)
  assert.doesNotMatch(app, /FirstWorkspaceGuideDialog/)
  assert.match(app, /const \[onboardingTourOpen, setOnboardingTourOpen\] = useState\(false\)/)
  assert.match(app, /getUiPreferences\(\)/)
  assert.match(app, /setUiPreferences\(\{ onboarding_tour_completed: true \}\)/)
  assert.match(app, /window\.addEventListener\("chawork:open-onboarding-tour"/)
  assert.match(tauri, /get_ui_preferences/)
  assert.match(tauri, /set_ui_preferences/)
  assert.match(globalSettingsCommands, /ui-preferences\.json/)
  assert.match(globalSettingsCommands, /onboarding_tour_completed/)
  assert.match(backendLib, /commands::global_settings::get_ui_preferences/)
  assert.match(backendLib, /commands::global_settings::set_ui_preferences/)
  assert.match(app, /const onboardingTourSteps = useMemo(?:<[^>]+>)?\(/)
  assert.match(app, /targetId: "settings-entry"/)
  assert.match(app, /targetId: "workspace-entry"/)
  assert.match(app, /targetId: "binding-general"/)
  assert.match(app, /targetId: "session-list"/)
  assert.match(app, /targetId: "employee-entry"/)
  assert.doesNotMatch(app, /targetId: "composer"/)
  assert.match(app, /const steps: OnboardingTourStep\[\] = \[/)
  assert.match(
    app,
    /id: "provider"[\s\S]*id: "workspace"[\s\S]*id: "binding"[\s\S]*id: "session"[\s\S]*id: "dream"[\s\S]*return steps/,
  )
  assert.doesNotMatch(app, /id: "composer"/)
  assert.match(app, /employeePanelOpen/)
  assert.match(app, /openDreamConfigPanel/)
  assert.match(app, /onboarding\.tour\.dream\.title/)
  assert.doesNotMatch(app, /globalProvider\?\.valid !== true \|\| settingsPanelOpen \|\| workspaceCascadeOpen/)
  assert.match(app, /<OnboardingTourOverlay/)
  assert.match(app, /steps=\{onboardingTourSteps\}/)
  assert.match(app, /const handleGeneralBindActionReady = useCallback/)
  assert.match(app, /setGeneralBindTourAction\(\(\) => action\)/)
  assert.match(app, /onGeneralBindActionReady=\{handleGeneralBindActionReady\}/)
  assert.match(app, /onboarding\.tour\.workspace\.title/)
  assert.match(app, /onboarding\.tour\.workspace\.action/)
  assert.match(app, /onboarding\.tour\.provider\.title/)
  assert.match(app, /openSettingsPanel\("provider"\)/)
  assert.match(app, /preferredEmployeeId=\{[\s\S]*workspaceCascadePreferredEmployeeId/)
  assert.match(app, /setWorkspaceCascadePreferredEmployeeId\("general"\)[\s\S]*setWorkspaceCascadeOpen\(true\)/)

  assert.match(tourOverlay, /export interface OnboardingTourStep/)
  assert.match(tourOverlay, /onboarding-tour-overlay/)
  assert.match(tourOverlay, /getBoundingClientRect/)
  assert.match(tourOverlay, /onboarding\.tour\.target_missing_hint/)
  assert.match(tourOverlay, /!targetRect \?/)
  assert.match(tourOverlay, /onPrevious/)
  assert.match(tourOverlay, /onNext/)
  assert.match(tourOverlay, /onSkip/)
  assert.doesNotMatch(tourOverlay, /onboarding\.tour\.composer\.title/)
})

test("top bar can manually reopen the onboarding tour", async () => {
  const topBar = await readProjectFile("frontend/components/layout/TopBar.tsx")
  const settingsPanel = await readProjectFile("frontend/components/settings/GlobalSettingsPanel.tsx")
  const rootConfigStore = await readProjectFile("frontend/stores/rootConfig.ts")
  const labels = await readProjectFile("frontend/lib/builtinLabels.ts")

  assert.doesNotMatch(topBar, /chawork:open-onboarding-tour/)
  assert.doesNotMatch(topBar, /onboarding\.tour\.reopen/)
  assert.doesNotMatch(settingsPanel, /role="tablist"/)
  assert.match(settingsPanel, /activeTab === "provider"/)
  assert.match(settingsPanel, /chawork:open-onboarding-tour/)
  assert.match(settingsPanel, /onboarding\.tour\.reopen/)
  assert.match(settingsPanel, /onboarding\.tour\.settings_title/)
  assert.match(settingsPanel, /onboarding\.tour\.settings_description/)
  assert.match(rootConfigStore, /settingsActiveTab: "provider"/)
  assert.match(rootConfigStore, /settingsActiveTab: tab \? mapGlobalSettingsTab\(tab\) : "provider"/)
  assert.match(rootConfigStore, /if \(tab === "provider" \|\| tab === "global_provider"\) return "provider"/)
  assert.match(labels, /"onboarding\.tour\.reopen"/)
  assert.match(labels, /"onboarding\.tour\.settings_title"/)
  assert.match(labels, /"onboarding\.tour\.settings_description"/)
})

test("tour anchors are attached to stable product controls", async () => {
  const [workspaceNav, bindingPrompt, sessionList, chatMain, composer, topBar] = await Promise.all([
    readProjectFile("frontend/components/workspace/WorkspaceNav.tsx"),
    readProjectFile("frontend/components/workspace/WorkspaceBindingPrompt.tsx"),
    readProjectFile("frontend/components/workspace/SessionList.tsx"),
    readProjectFile("frontend/components/chat/ChatMain.tsx"),
    readProjectFile("frontend/components/chat/Composer.tsx"),
    readProjectFile("frontend/components/layout/TopBar.tsx"),
  ])

  assert.match(topBar, /data-tour-id="settings-entry"/)
  assert.match(topBar, /data-tour-id="employee-entry"/)
  assert.match(workspaceNav, /data-tour-id="workspace-entry"/)
  assert.match(bindingPrompt, /data-tour-id="binding-general"/)
  assert.match(sessionList, /data-tour-id="session-list"/)
  assert.doesNotMatch(chatMain, /composerCoachMark/)
  assert.match(composer, /data-tour-id="composer"/)
  assert.doesNotMatch(composer, /OnboardingCoachMark/)
})

test("unbound workspace prompt prioritizes using the general employee", async () => {
  const prompt = await readProjectFile("frontend/components/workspace/WorkspaceBindingPrompt.tsx")

  assert.match(prompt, /const generalEmployee = ordinaryEmployees\.find\(\(e\) => e\.id === "general"\)/)
  assert.match(prompt, /binding\.status === "unbound"/)
  assert.match(prompt, /handleBind\("general"\)/)
  assert.match(prompt, /onboarding\.binding\.use_general/)
  assert.match(prompt, /onboarding\.binding\.other_employee/)
  assert.match(prompt, /loadEmployees\(\)/)
})

test("cascade and empty states explain the recommended general employee path", async () => {
  const [app, cascade, builtinLabels, enLabels] = await Promise.all([
    readProjectFile("frontend/App.tsx"),
    readProjectFile("frontend/components/workspace/EmployeeWorkspaceCascadeDialog.tsx"),
    readProjectFile("frontend/lib/builtinLabels.ts"),
    readProjectFile("frontend/lib/localeLabels/en-US.ts"),
  ])

  assert.match(app, /chat\.empty\.pick_workspace/)
  assert.match(app, /通用员工/)
  assert.match(app, /setWorkspaceCascadePreferredEmployeeId\("general"\)/)

  assert.match(cascade, /workspace\.cascade\.general_recommended_badge/)
  assert.match(cascade, /employee\.id === "general"/)
  assert.match(cascade, /推荐 · 首次使用/)

  for (const labels of [builtinLabels, enLabels]) {
    assert.match(labels, /"onboarding\.tour\.workspace\.title"/)
    assert.match(labels, /"onboarding\.tour\.provider\.title"/)
    assert.match(labels, /"onboarding\.tour\.provider\.action"/)
    assert.match(labels, /"onboarding\.tour\.workspace\.action"/)
    assert.match(labels, /"onboarding\.tour\.binding\.title"/)
    assert.match(labels, /"onboarding\.tour\.session\.title"/)
    assert.match(labels, /"onboarding\.tour\.dream\.title"/)
    assert.match(labels, /"onboarding\.tour\.dream\.action"/)
    assert.doesNotMatch(labels, /"onboarding\.tour\.composer\.title"/)
    assert.match(labels, /"onboarding\.tour\.skip"/)
    assert.match(labels, /"onboarding\.tour\.previous"/)
    assert.match(labels, /"onboarding\.tour\.next"/)
    assert.match(labels, /"onboarding\.tour\.done"/)
    assert.match(labels, /"onboarding\.tour\.skip_aria"/)
    assert.match(labels, /"onboarding\.tour\.target_missing_hint"/)
    assert.match(labels, /"onboarding\.binding\.use_general"/)
    assert.match(labels, /"workspace\.cascade\.general_recommended_badge"/)
  }
})
