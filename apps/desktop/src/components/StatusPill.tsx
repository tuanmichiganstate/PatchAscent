import type { ReactNode } from "react";

interface StatusPillProps {
  readonly tone: "ready" | "pending" | "blocked" | "neutral";
  readonly children: ReactNode;
}

export function StatusPill({ tone, children }: StatusPillProps) {
  return <span className={`status-pill status-pill--${tone}`}>{children}</span>;
}
