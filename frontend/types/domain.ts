export interface DomainManifest {
  id: string
  name: string
  description?: string
  object_label?: string
  object_plural_label?: string
  default_object_type?: string
  primary_workflows: string[]
}

export interface TemplateEntry {
  name: string
  filename: string
  content: string
}

export interface SkillMeta {
  name: string
  description: string
  dir_name: string
}

export interface UiLabels {
  [key: string]: unknown
}

export interface DomainPack {
  manifest: DomainManifest
  agents_md: string | null
  objects_schema: unknown | null
  workflows: unknown | null
  templates: TemplateEntry[]
  skills: SkillMeta[]
  labels: UiLabels | null
}
