import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, existsSync, mkdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const workspace = resolve(scriptDirectory, '..')
const args = process.argv.slice(2)
const debug = args.includes('--debug')
const relayOnly = args.includes('--relay-only')
const targetIndex = args.indexOf('--target')

/** Cargo packages copied into `src-tauri/binaries/` for Tauri `externalBin`. */
const ALL_SIDECARS = [
  { package: 'notch-hook', binary: 'llm-notch-hook' },
  { package: 'notch-remote', binary: 'llm-notch-relay' },
]

/** Remote-deploy relay triples built in CI/release (SSH targets only; Windows remote is unsupported). */
const REMOTE_RELAY_TARGETS = [
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'x86_64-apple-darwin',
  'aarch64-apple-darwin',
]

if (process.argv.includes('--list-remote-relay-targets')) {
  for (const triple of REMOTE_RELAY_TARGETS) {
    console.log(triple)
  }
  process.exit(0)
}

const sidecars = relayOnly
  ? ALL_SIDECARS.filter(({ binary }) => binary === 'llm-notch-relay')
  : ALL_SIDECARS

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
const extension = target.includes('windows') ? '.exe' : ''
const destinationDirectory = join(workspace, 'src-tauri', 'binaries')

function sidecarDestination(binary) {
  return join(destinationDirectory, `${binary}-${target}${extension}`)
}

function sidecarSource(binary) {
  return join(workspace, 'target', target, profile, `${binary}${extension}`)
}

if (process.env.LLM_NOTCH_SKIP_HELPER_BUILD === '1') {
  for (const { binary } of sidecars) {
    const destination = sidecarDestination(binary)
    if (!existsSync(destination)) {
      throw new Error(`Prepared sidecar is missing: ${destination}`)
    }
    console.log(`Reusing prepared sidecar: ${destination}`)
  }
  process.exit(0)
}

for (const { package: cratePackage } of sidecars) {
  const cargoArgs = ['build', '-p', cratePackage, '--target', target]
  if (!debug) cargoArgs.push('--release')

  const build = spawnSync('cargo', cargoArgs, {
    cwd: workspace,
    stdio: 'inherit',
  })
  if (build.status !== 0) {
    process.exit(build.status ?? 1)
  }
}

mkdirSync(destinationDirectory, { recursive: true })
for (const { binary } of sidecars) {
  const source = sidecarSource(binary)
  const destination = sidecarDestination(binary)
  copyFileSync(source, destination)
  if (!extension) chmodSync(destination, 0o755)
  console.log(`Prepared native sidecar: ${destination}`)
}
