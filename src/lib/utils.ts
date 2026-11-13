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

/** Interpreta o resultado real do backfill: sucesso, parcial ou falha — não só pelo exit code. */
export function backfillOutcome(
  run: { output: { exitCode?: number | null; cancelled: boolean; stdout: string; stderr: string }; parsed: { kind: "ok"; data: { selected: number; importedSessions: number; failedSessions: number; importedEvents: number } } | { kind: "raw"; reason: string } },
  maxSessions: number,
): { tone: Tone; title: string; notes: string[] } {
  const notes: string[] = [];
  for (const line of `${run.output.stdout}\n${run.output.stderr}`.split("\n")) {
    const l = line.trim();
    if (!l.startsWith("ai-memory:")) continue;
    if (l.includes("dropped_policy"))
      notes.push(`${l.replace(/^ai-memory:\s*/, "")} → descartado pela política de captura (sanitização/filtro). O transcript local foi mantido; repetir não muda o resultado.`);
    else if (l.includes("oversized")) notes.push(`${l.replace(/^ai-memory:\s*/, "")} → registros grandes demais foram pulados; o restante da sessão entrou.`);
    else notes.push(l.replace(/^ai-memory:\s*/, ""));
  }
  if (run.output.cancelled) return { tone: "warn", title: "Importação cancelada.", notes };
  if (run.parsed.kind !== "ok") return { tone: run.output.exitCode === 0 ? "warn" : "err", title: `Resultado não interpretado (${run.parsed.reason}).`, notes };
  const d = run.parsed.data;
  if (d.selected >= maxSessions)
    notes.push(`O limite de ${maxSessions} sessões foi atingido: pode haver sessões locais mais antigas. Para trazê-las, aumente "Máx. sessões", marque "Forçar" e simule de novo (o ai-memory deduplica).`);
  if (d.importedSessions > 0 && d.failedSessions > 0)
    return { tone: "warn", title: `Importação parcial: ${d.importedSessions} sessão(ões) e ${d.importedEvents} eventos importados; ${d.failedSessions} sessão(ões) não importada(s).`, notes };
  if (d.importedSessions > 0) return { tone: "ok", title: `Importação concluída: ${d.importedSessions} sessão(ões), ${d.importedEvents} eventos.`, notes };
  if (d.failedSessions > 0) return { tone: "err", title: `Nenhuma sessão importada; ${d.failedSessions} falharam.`, notes };
  return { tone: run.output.exitCode === 0 ? "info" : "err", title: "Nada novo para importar.", notes };
}

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
