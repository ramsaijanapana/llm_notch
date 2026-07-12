export type NativeClientErrorCode =
  | 'protocol-incompatible'
  | 'invoke-failed'
  | 'stream-gap'
  | 'stream-closed'
  | 'resync-required'
  | 'not-available'
  | 'invalid-response'
  | 'remote-host-invalid'
  | 'remote-host-missing'
  | 'remote-backend-unavailable'

export class NativeClientError extends Error {
  override readonly name = 'NativeClientError'
  readonly code: NativeClientErrorCode

  constructor(code: NativeClientErrorCode, message: string, cause?: unknown) {
    super(message, cause !== undefined ? { cause } : undefined)
    this.code = code
  }
}

export function isNativeClientError(error: unknown): error is NativeClientError {
  return error instanceof NativeClientError
}
