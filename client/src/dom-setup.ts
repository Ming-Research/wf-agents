// What the page's tests that draw in a DOM (`// @vitest-environment happy-dom` at the top of the file) import first:
// React's switch that says updates happen inside `act`, and the part of a browser that happy-dom does not have and the
// chart uses. A test that needs more of a browser than this adds it here, once, not in the test.

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (typeof Element.prototype.scrollIntoView === 'undefined') {
  Element.prototype.scrollIntoView = () => undefined;
}
