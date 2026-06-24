import assert from "node:assert/strict"
import test from "node:test"

import { formatDisplayPath, normalizePathKey } from "./formatPath.ts"

test("normalizePathKey strips Windows extended prefix and lowercases drive", () => {
  assert.equal(
    normalizePathKey("\\\\?\\D:\\测试222"),
    "d:/测试222",
  )
  assert.equal(
    normalizePathKey("/Users/mac/测试"),
    "/Users/mac/测试",
  )
})

test("formatDisplayPath removes extended prefix for display", () => {
  assert.equal(formatDisplayPath("\\\\?\\D:\\测试222"), "D:\\测试222")
  assert.equal(formatDisplayPath("/Users/mac/测试"), "/Users/mac/测试")
})
