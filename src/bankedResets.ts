/** Relative redemption deadline; dates remain server-owned and never inferred. */
export function bankedResetExpiry(value: string | null, now: number): string {
  if (!value) return "No expiration";
  const expires = Date.parse(value);
  if (!Number.isFinite(expires)) return "Expiration unavailable";
  const remaining = expires - now;
  if (remaining <= 0) return "Expired";
  const minutes = Math.floor(remaining / 60_000);
  if (minutes < 1) return "Expires in less than a minute";
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor(minutes / 60) % 24;
  if (days > 0) return `Expires in ${days}d${hours ? ` ${hours}h` : ""}`;
  const rest = minutes % 60;
  if (hours > 0) return `Expires in ${hours}h${rest ? ` ${rest}m` : ""}`;
  return `Expires in ${minutes}m`;
}
