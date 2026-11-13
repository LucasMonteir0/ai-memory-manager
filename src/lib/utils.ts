import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { IntegrationStatus } from "./types";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function fmtDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

export function fmtNum(n?: number | null): string {
  return n == null ? "—" : n.toLocaleString("pt-BR");
}

export function fmtAge(ms?: number | null): string {
  if (ms == null) return "—";
  if (ms < 1000) return `${ms} ms`;
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  return `${Math.round(m / 60)} h`;
}

export function fmtBytes(n?: number | null): string {
  if (n == null) return "—";
  const u = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i ? 1 : 0)} ${u[i]}`;
}

export type Tone = "ok" | "warn" | "err" | "info" | "muted";

export const integrationLabel: Record<IntegrationStatus, { label: string; tone: Tone }> = {
  not_detected: { label: "Não detectado", tone: "muted" },
  not_configured: { label: "Não configurado", tone: "muted" },
  configured: { label: "Configurado", tone: "info" },
  verified: { label: "Testado", tone: "ok" },
  untrusted: { label: "Não autorizado", tone: "warn" },
  partial: { label: "Parcial", tone: "warn" },
  error: { label: "Erro", tone: "err" },
  unsupported: { label: "Indisponível", tone: "muted" },
};

export const agentLabel: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
  "claude-desktop": "Claude Chat",
  "copilot-cli": "Copilot CLI",
};
