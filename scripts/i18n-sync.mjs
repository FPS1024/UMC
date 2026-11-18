import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const sourceRoot = join(root, 'docs/en')
const targetRoot = join(root, 'docs/zh')

async function markdownFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) {
      files.push(...(await markdownFiles(path)))
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      files.push(path)
    }
  }
  return files
}

async function exists(path) {
  try {
    await stat(path)
    return true
  } catch {
    return false
  }
}

function splitFrontmatter(text) {
  const match = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/)
  if (!match) {
    return { frontmatter: '', body: text }
  }
  return { frontmatter: match[1], body: text.slice(match[0].length) }
}

function withTranslationMeta(frontmatter, relativePath) {
  const lines = frontmatter
    ? frontmatter
        .split(/\r?\n/)
        .filter((line) => !/^(translation|source)\s*:/.test(line.trim()))
    : []
  lines.push('translation: pending')
  lines.push(`source: en/${relativePath.replaceAll('\\', '/')}`)
  return lines.join('\n')
}

const files = await markdownFiles(sourceRoot)
let created = 0
let existing = 0

for (const file of files) {
  const relativePath = relative(sourceRoot, file)
  const target = join(targetRoot, relativePath)

  if (await exists(target)) {
    existing += 1
    continue
  }

  let content = await readFile(file, 'utf8')
  content = content
    .replaceAll('](/en/', '](/zh/')
    .replaceAll('href="/en/', 'href="/zh/')
  const { frontmatter, body } = splitFrontmatter(content)
  const output = `---\n${withTranslationMeta(frontmatter, relativePath)}\n---\n\n${body.replace(/^\s+/, '')}`

  await mkdir(dirname(target), { recursive: true })
  await writeFile(target, output)
  created += 1
  console.log(`created docs/zh/${relativePath}`)
}

console.log(
  `i18n sync: ${created} created, ${existing} existing, ${files.length} English pages`,
)
