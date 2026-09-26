import "@testing-library/jest-dom/vitest";

// Date/Intl assertions in format tests are written against UTC.
// @ts-expect-error type error without @types/node package
process.env.TZ = "UTC";
