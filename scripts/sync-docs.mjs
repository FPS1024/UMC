import { copyFile, mkdir } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const protocols = [
  ['rpc-v1.md', 'rpc-v1.md'],
  ['auth-v1.md', 'auth-v1.md'],
]

for (const [sourceName, destinationName] of protocols) {
  const source = resolve(root, 'protocols', sourceName)
  const destination = resolve(root, 'docs/en/reference', destinationName)
  await mkdir(dirname(destination), { recursive: true })
  await copyFile(source, destination)
  console.log(`Synced protocols/${sourceName} -> docs/en/reference/${destinationName}`)
}
