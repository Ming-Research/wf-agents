// What the page's tests that draw in a DOM (`// @vitest-environment happy-dom` at the top of the file) import first:
// the parts of a browser that happy-dom does not have and Primer's components use, and React's switch that says updates
// happen inside `act`. Brought over from the page of the engine on `main`, keeping only what this page needs.

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (window.matchMedia === undefined) {
  // Nothing matches, and nothing changes.
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

if (typeof Element.prototype.scrollIntoView === 'undefined') {
  Element.prototype.scrollIntoView = () => undefined;
}
