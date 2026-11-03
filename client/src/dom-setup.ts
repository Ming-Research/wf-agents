// What the page's tests that draw in a DOM (`// @vitest-environment happy-dom` at the top of the file) import first: the
// parts of a browser that happy-dom does not have and Primer's components or the page itself use, and React's switch
// that says updates happen inside `act`. Each part is added only where happy-dom lacks it. A test that needs more of a
// browser than this adds it here, once, not in the test.

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (window.matchMedia === undefined) {
  // Nothing matches, and nothing changes: the width of the page is the widest.
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

/// An observer that observes nothing: no layout is done, so there is nothing to report.
class Unobserving {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
  takeRecords(): [] {
    return [];
  }
}

if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = Unobserving as unknown as typeof ResizeObserver;
}
if (typeof globalThis.IntersectionObserver === 'undefined') {
  globalThis.IntersectionObserver = Unobserving as unknown as typeof IntersectionObserver;
}
if (typeof Element.prototype.scrollTo === 'undefined') {
  Element.prototype.scrollTo = () => undefined;
}
if (typeof Element.prototype.scrollIntoView === 'undefined') {
  Element.prototype.scrollIntoView = () => undefined;
}
