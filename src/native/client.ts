import { isTauriEnvironment } from './environment.ts'
import { createFakeNativeClient } from './FakeNativeClient.ts'
import { createTauriNativeClient } from './TauriNativeClient.ts'
import type { CreateNativeClientOptions, NativeClient } from './types.ts'

export function createNativeClient(options: CreateNativeClientOptions = {}): NativeClient {
  if (!options.forcePreview && isTauriEnvironment()) {
    return createTauriNativeClient()
  }

  return createFakeNativeClient()
}

export type { NativeClient } from './types.ts'
