import type { FakeNativeClient } from './FakeNativeClient.ts'
import { assertPreviewClient } from './FakeNativeClient.ts'
import { isNativePreviewActive } from './previewRouting.ts'
import type { NativeClient } from './types.ts'

export interface NativePreviewBridge {
  simulateResync: (reason?: string) => void
  simulateSequenceGap: () => void
}

declare global {
  interface Window {
    __LLM_NOTCH_PREVIEW__?: NativePreviewBridge
  }
}

/** Registers test-only preview hooks; never called in production Tauri mode. */
export function installPreviewBridge(client: NativeClient): void {
  if (client.mode !== 'preview' || !isNativePreviewActive() || typeof window === 'undefined') {
    return
  }

  assertPreviewClient(client)
  const previewClient = client as FakeNativeClient

  window.__LLM_NOTCH_PREVIEW__ = {
    simulateResync: (reason = 'Preview resync requested') => {
      previewClient.simulateResyncRequired(reason)
    },
    simulateSequenceGap: () => {
      previewClient.simulateSequenceGap()
    },
  }
}

export function clearPreviewBridge(): void {
  if (typeof window !== 'undefined') {
    delete window.__LLM_NOTCH_PREVIEW__
  }
}
