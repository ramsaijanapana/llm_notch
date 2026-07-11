import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './app/App.tsx'
import { DesktopApp } from './app/DesktopApp.tsx'
import { isTauriEnvironment } from './native/environment.ts'
import { isNativePreviewActive } from './native/previewRouting.ts'
import './styles/global.css'

const rootElement = document.getElementById('root')

if (!rootElement) {
  throw new Error('Root element #root not found')
}

const renderNativeShell = isTauriEnvironment() || isNativePreviewActive()

createRoot(rootElement).render(
  <StrictMode>{renderNativeShell ? <DesktopApp /> : <App />}</StrictMode>,
)
