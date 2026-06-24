import assert from "node:assert/strict"
import test from "node:test"

import { isValidEmployeeId, slugify } from "./slugify.ts"

test("slugify preserves CJK and collapses separators", () => {
  assert.equal(slugify("Hello World"), "hello-world")
  assert.equal(slugify("罗伯特"), "罗伯特")
  assert.equal(slugify("黄一鸣 笔记"), "黄一鸣-笔记")
  assert.equal(slugify("ロバート"), "ロバート")
  assert.equal(slugify("###"), "untitled")
})

test("isValidEmployeeId accepts CJK ids and rejects uppercase ascii", () => {
  assert.equal(isValidEmployeeId("罗伯特"), true)
  assert.equal(isValidEmployeeId("ip-screening"), true)
  assert.equal(isValidEmployeeId("HAS_UPPER"), false)
})
