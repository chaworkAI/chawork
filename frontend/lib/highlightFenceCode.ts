import hljs from "highlight.js/lib/core"
import type { LanguageFn } from "highlight.js"
import bash from "highlight.js/lib/languages/bash"
import css from "highlight.js/lib/languages/css"
import diff from "highlight.js/lib/languages/diff"
import javascript from "highlight.js/lib/languages/javascript"
import json from "highlight.js/lib/languages/json"
import markdown from "highlight.js/lib/languages/markdown"
import plaintext from "highlight.js/lib/languages/plaintext"
import python from "highlight.js/lib/languages/python"
import rust from "highlight.js/lib/languages/rust"
import shell from "highlight.js/lib/languages/shell"
import typescript from "highlight.js/lib/languages/typescript"
import xml from "highlight.js/lib/languages/xml"
import yaml from "highlight.js/lib/languages/yaml"

import "highlight.js/styles/github.min.css"

const reg = (name: string, fn: LanguageFn) => {
  hljs.registerLanguage(name, fn)
}

reg("bash", bash)
reg("sh", bash)
reg("shell", shell)
reg("zsh", shell)
reg("css", css)
reg("diff", diff)
reg("javascript", javascript)
reg("js", javascript)
reg("json", json)
reg("markdown", markdown)
reg("md", markdown)
reg("plaintext", plaintext)
reg("text", plaintext)
reg("txt", plaintext)
reg("python", python)
reg("py", python)
reg("rust", rust)
reg("rs", rust)
reg("typescript", typescript)
reg("ts", typescript)
reg("tsx", typescript)
reg("jsx", javascript)
reg("xml", xml)
reg("html", xml)
reg("vue", xml)
reg("yaml", yaml)
reg("yml", yaml)

function normalizeLang(lang?: string): string {
  if (!lang?.trim()) return "plaintext"
  return lang.trim().toLowerCase()
}

/**
 * Returns HTML for a fenced code block (hljs spans). Caller must render with `dangerouslySetInnerHTML`.
 * Falls back to `plaintext` when the language id is unknown.
 */
export function highlightFenceCodeToHtml(code: string, lang?: string): string {
  const id = normalizeLang(lang)
  const language = hljs.getLanguage(id) ? id : "plaintext"
  const trimmed = code.replace(/\n+$/, "")
  try {
    return hljs.highlight(trimmed, { language }).value
  } catch {
    return hljs.highlight(trimmed, { language: "plaintext" }).value
  }
}
