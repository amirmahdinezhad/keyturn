import "@testing-library/jest-dom/vitest";

// JSDOM has no layout observer; Radix measures slider thumbs with this API.
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// Select uses pointer capture and scrolls its active option in real webviews.
HTMLElement.prototype.hasPointerCapture = () => false;
HTMLElement.prototype.setPointerCapture = () => {};
HTMLElement.prototype.releasePointerCapture = () => {};
HTMLElement.prototype.scrollIntoView = () => {};

// Date/Intl assertions in format tests are written against UTC.
// @ts-expect-error type error without @types/node package
process.env.TZ = "UTC";
