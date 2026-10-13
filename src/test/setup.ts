import "@testing-library/jest-dom/vitest";

// JSDOM has no layout observer; Radix measures slider thumbs with this API.
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// Date/Intl assertions in format tests are written against UTC.
// @ts-expect-error type error without @types/node package
process.env.TZ = "UTC";
