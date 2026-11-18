import { copyFile, mkdir } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const source = resolve(root, 'protocols/rpc-v1.md')
const destination = resolve(root, 'docs/en/reference/rpc-v1.md')

await mkdir(dirname(destination), { recursive: true })
await copyFile(source, destination)
console.log('Synced protocols/rpc-v1.md -> docs/en/reference/rpc-v1.md')
