import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const workspace = resolve(scriptDirectory, '..')
const args = process.argv.slice(2)
const debug = args.includes('--debug')
const targetIndex = args.indexOf('--target')

function commandOutput(command, commandArgs) {
  const result = spawnSync(command, commandArgs, {
    cwd: workspace,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
  return result.stdout
}

function hostTriple() {
  const version = commandOutput('rustc', ['-vV'])
  const hostLine = version.split(/\r?\n/).find((line) => line.startsWith('host: '))
  if (!hostLine) {
    throw new Error('rustc did not report a host target triple')
  }
  return hostLine.slice('host: '.length).trim()
}

const explicitTarget =
  targetIndex >= 0 ? args[targetIndex + 1] : process.env.TAURI_ENV_TARGET_TRIPLE
if (targetIndex >= 0 && !explicitTarget) {
  throw new Error('--target requires a Rust target triple')
}

const target = explicitTarget || hostTriple()
const profile = debug ? 'debug' : 'release'
const cargoArgs = ['build', '-p', 'notch-hook', '--target', target]
if (!debug) cargoArgs.push('--release')

const build = spawnSync('cargo', cargoArgs, {
  cwd: workspace,
  stdio: 'inherit',
})
if (build.status !== 0) {
  process.exit(build.status ?? 1)
}

const extension = target.includes('windows') ? '.exe' : ''
const source = join(workspace, 'target', target, profile, `llm-notch-hook${extension}`)
const destinationDirectory = join(workspace, 'src-tauri', 'binaries')
const destination = join(destinationDirectory, `llm-notch-hook-${target}${extension}`)
mkdirSync(destinationDirectory, { recursive: true })
copyFileSync(source, destination)
if (!extension) chmodSync(destination, 0o755)

console.log(`Prepared native helper: ${destination}`)
