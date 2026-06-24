import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import test from "node:test"
import ts from "typescript"

async function importTs(path) {
  const source = await readFile(new URL(path, import.meta.url), "utf8")
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2021,
      verbatimModuleSyntax: true,
    },
  })
  return import(`data:text/javascript,${encodeURIComponent(outputText)}`)
}

test("composer enter sends only plain non-composing Enter", async () => {
  const { shouldSendComposerMessage } = await importTs("./composerKeyboard.ts")

  assert.equal(
    shouldSendComposerMessage({
      key: "Enter",
      shiftKey: false,
      sendBlocked: false,
      nativeEvent: { isComposing: false },
    }),
    true,
  )
  assert.equal(
    shouldSendComposerMessage({
      key: "Enter",
      shiftKey: true,
      sendBlocked: false,
      nativeEvent: { isComposing: false },
    }),
    false,
  )
  assert.equal(
    shouldSendComposerMessage({
      key: "Enter",
      shiftKey: false,
      sendBlocked: true,
      nativeEvent: { isComposing: false },
    }),
    false,
  )
})

test("composer enter ignores IME composition confirmation", async () => {
  const { shouldSendComposerMessage } = await importTs("./composerKeyboard.ts")

  assert.equal(
    shouldSendComposerMessage({
      key: "Enter",
      shiftKey: false,
      sendBlocked: false,
      nativeEvent: { isComposing: true },
    }),
    false,
  )
  assert.equal(
    shouldSendComposerMessage({
      key: "Enter",
      shiftKey: false,
      sendBlocked: false,
      nativeEvent: { isComposing: false, keyCode: 229 },
    }),
    false,
  )
  assert.equal(
    shouldSendComposerMessage({
      key: "Process",
      shiftKey: false,
      sendBlocked: false,
      nativeEvent: { isComposing: false },
    }),
    false,
  )
})
