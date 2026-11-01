import { expect, it } from "vitest";
import { bankedResetExpiry } from "./bankedResets";
const now = Date.parse("2026-11-01T12:00:00Z");
it("uses the actual remaining redemption time instead of a calendar date", () => {
  expect(bankedResetExpiry("2026-11-30T16:00:00Z", now)).toBe("Expires in 29d 4h");
  expect(bankedResetExpiry("2026-11-01T14:09:00Z", now)).toBe("Expires in 2h 9m");
  expect(bankedResetExpiry("2026-11-01T12:04:00Z", now)).toBe("Expires in 4m");
  expect(bankedResetExpiry("2026-11-01T12:00:30Z", now)).toBe("Expires in less than a minute");
  expect(bankedResetExpiry("2026-11-01T12:00:00Z", now)).toBe("Expired");
});
it("keeps absent and invalid expiry distinct", () => {
  expect(bankedResetExpiry(null, now)).toBe("No expiration");
  expect(bankedResetExpiry("invalid", now)).toBe("Expiration unavailable");
});
