import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"

async function readProjectFile(path) {
  return readFile(new URL(`../../${path}`, import.meta.url), "utf8")
}

test("top bar guards Tauri window APIs outside the desktop webview", async () => {
  const topBar = await readProjectFile("frontend/components/layout/TopBar.tsx")

  assert.match(topBar, /function getOptionalCurrentWindow\(\)/)
  assert.match(topBar, /try \{[\s\S]*return getCurrentWindow\(\)[\s\S]*\} catch/)
  assert.match(topBar, /if \(!appWindow\) return/)
  assert.doesNotMatch(topBar, /const appWindow = getCurrentWindow\(\)/)
})
