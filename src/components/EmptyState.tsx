/** Block empty states — the dashed box for no providers / load errors. */
export function EmptyState({ message }: { message: string }) {
  return <p className="empty">{message}</p>;
}
