import { Badge } from "./ui";
import { integrationLabel, type Tone } from "@/lib/utils";
import type { DetectionState, IntegrationStatus } from "@/lib/types";

export function IntegrationBadge({ status }: { status: IntegrationStatus }) {
  const s = integrationLabel[status] ?? { label: status, tone: "muted" as Tone };
  return <Badge tone={s.tone}>{s.label}</Badge>;
}

export const detectionTone: Record<DetectionState, Tone> = {
  healthy: "ok",
  server_stopped: "warn",
  no_setup: "warn",
  auth_error: "err",
  invalid: "err",
  unsupported_version: "err",
  not_installed: "muted",
  unknown: "warn",
};
