/**
 * Vitest global test setup.
 * Extends expect with @testing-library/jest-dom matchers for DOM assertions.
 */
import '@testing-library/jest-dom/vitest'

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
})

Element.prototype.scrollIntoView = () => {}
