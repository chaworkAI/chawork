import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"

async function readProjectFile(path) {
  return readFile(new URL(`../../${path}`, import.meta.url), "utf8")
}

test("bound workspace skill manager routes directly to employee skills", async () => {
  const [employeeStore, skillStore] = await Promise.all([
    readProjectFile("frontend/stores/employee.ts"),
    readProjectFile("frontend/stores/skill.ts"),
  ])

  assert.match(employeeStore, /openEmployeeSkills:\s*\(employeeId:\s*string\)\s*=>\s*Promise<void>/)
  assert.match(employeeStore, /openEmployeeSkills:\s*async\s*\(employeeId\)/)
  assert.match(employeeStore, /activeTab:\s*"skills"/)

  assert.match(skillStore, /binding\?\.status === "bound" && binding\.employee_id/)
  assert.match(
    skillStore,
    /useEmployeeStore\.getState\(\)\.openEmployeeSkills\(binding\.employee_id\)/,
  )
  assert.doesNotMatch(skillStore, /binding\?\.status === "bound"\)\s*\{\s*useEmployeeStore\.getState\(\)\.openPanel\(\)/)
})

test("employee skill copy UI is presented as adding a skill", async () => {
  const employeeSkills = await readProjectFile("frontend/components/employee/EmployeeSkills.tsx")

  assert.match(employeeSkills, /t\("employee\.skills\.copy_from_root", "添加 Skill"\)/)
  assert.match(employeeSkills, /t\("employee\.skills\.picker_title", "从 Root 添加 Skill"\)/)
  assert.match(employeeSkills, /copyingSkillId/)
  assert.match(employeeSkills, /grid-cols-1 gap-3[\s\S]*md:grid-cols-2[\s\S]*xl:grid-cols-3/)
  assert.match(employeeSkills, /t\("employee\.skills\.source_root", "Root"\)/)
  assert.match(employeeSkills, /copyingSkillId === s\.id/)
  assert.match(employeeSkills, /size="xs"[\s\S]*t\("employee\.skills\.copy_action", "添加"\)/)
  assert.match(employeeSkills, /t\("employee\.skills\.status_enabled", "已启用"\)/)
  assert.match(employeeSkills, /t\("employee\.skills\.status_disabled", "已禁用"\)/)
  assert.match(employeeSkills, /grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3[\s\S]*skills\.map\(\(skill\) =>[\s\S]*grid min-h-\[150px\]/)
  assert.match(employeeSkills, /t\("employee\.skills\.empty", "暂无 Skill，点击上方按钮添加"\)/)
  assert.match(employeeSkills, /await copySkill\(selectedEmployeeId, skillId\)/)
})

test("hub skill install remains root-only and does not auto-copy to employee", async () => {
  const hubStore = await readProjectFile("frontend/stores/hub.ts")

  assert.match(hubStore, /useWorkspaceStore/)
  assert.match(hubStore, /activeBinding\?\.status === "bound"/)
  assert.match(hubStore, /可通过 Skill 管理添加给当前员工/)
  assert.match(hubStore, /message:\s*"下载完成，已写入 Root 技能库。"/)
  assert.match(hubStore, /Skill 已下载到 Root/)
  assert.doesNotMatch(hubStore, /copyRootSkillToEmployee|copySkill\(/)
})
