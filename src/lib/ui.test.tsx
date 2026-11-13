import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { alertsFrom } from "@/pages/Overview";
import { IntegrationBadge } from "@/components/StatusBadge";
import { backfillOutcome, fmtAge, integrationLabel } from "@/lib/utils";
import type { AgentsView, Detection } from "@/lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

const baseDet = (over: Partial<Detection>): Detection => ({
  state: "healthy",
  stateLabel: "",
  candidates: [],
  commands: [],
  hasRecover: true,
  hasHookDrain: false,
  dataDir: "",
  config: { configPath: "", exists: true },
  serverUrl: "",
  webUrl: "",
  tokenAvailable: true,
  checkedAt: "",
  ...over,
});

describe("alertas do dashboard", () => {
  it("servidor parado avisa sobre spool, sem dizer que está tudo OK", () => {
    const a = alertsFrom(baseDet({ state: "server_stopped" }));
    expect(a.some((x) => x.text.includes("spool"))).toBe(true);
  });

  it("status não interpretado gera alerta (nunca assume saudável)", () => {
    const a = alertsFrom(baseDet({ state: "unknown" }));
    expect(a.some((x) => x.text.includes("não interpretada"))).toBe(true);
  });

  it("hooks não autorizados e MCP com erro viram alertas acionáveis", () => {
    const comp = (status: any, detail = "") => ({ status, detail, evidence: [], canConfigure: true, canDisable: true });
    const view = {
      apps: {} as any,
      captureTests: [],
      agents: [
        { id: "codex", displayName: "Codex Desktop", app: { installed: true }, mcp: comp("error", "variável ausente"), hooks: comp("untrusted") },
      ],
    } as unknown as AgentsView;
    const a = alertsFrom(baseDet({}), view);
    expect(a.find((x) => x.text.includes("variável ausente"))?.tone).toBe("err");
    expect(a.find((x) => x.text.includes("não autorizados"))?.go).toBe("agents");
  });

  it("spool atrasado só alerta acima de 10 min", () => {
    const st = (age: number) => baseDet({ status: { kind: "ok", data: { spoolPending: 3, spoolOldestAgeMs: age } } });
    expect(alertsFrom(st(5_000)).length).toBe(0);
    expect(alertsFrom(st(20 * 60_000)).some((x) => x.text.includes("Spool atrasado"))).toBe(true);
  });
});

describe("resultado do backfill", () => {
  const run = (exitCode: number, data: any, stderr = "") => ({
    output: { exitCode, cancelled: false, stdout: "", stderr },
    parsed: { kind: "ok" as const, data },
  });
  it("24 importadas + 1 falha é parcial (aviso), não erro — caso real", () => {
    const o = backfillOutcome(
      run(1, { selected: 25, importedSessions: 24, importedEvents: 432, failedSessions: 1 },
        "ai-memory: backfill of claude session 9b78405b failed: hook item 1 was not durably stored (dropped_policy); keeping transcript\nai-memory: claude session c8a73dc4: skipped 2 oversized transcript record(s); the rest was imported"),
      25,
    );
    expect(o.tone).toBe("warn");
    expect(o.title).toContain("parcial");
    expect(o.notes.some((n) => n.includes("política de captura"))).toBe(true);
    expect(o.notes.some((n) => n.includes("grandes demais"))).toBe(true);
    expect(o.notes.some((n) => n.includes("limite de 25"))).toBe(true);
  });
  it("tudo importado é sucesso; nada importado com falhas é erro", () => {
    expect(backfillOutcome(run(0, { selected: 3, importedSessions: 3, importedEvents: 10, failedSessions: 0 }), 25).tone).toBe("ok");
    expect(backfillOutcome(run(1, { selected: 2, importedSessions: 0, importedEvents: 0, failedSessions: 2 }), 25).tone).toBe("err");
  });
});

describe("rótulos de integração", () => {
  it("todos os estados do modelo têm rótulo pt-BR", () => {
    for (const k of ["not_detected", "not_configured", "configured", "verified", "untrusted", "partial", "error", "unsupported"] as const) {
      expect(integrationLabel[k].label.length).toBeGreaterThan(2);
    }
  });
  it("renderiza 'Não autorizado' para untrusted", () => {
    render(<IntegrationBadge status="untrusted" />);
    expect(screen.getByText("Não autorizado")).toBeInTheDocument();
  });
  it("formata idade", () => {
    expect(fmtAge(1500)).toBe("2 s");
    expect(fmtAge(3 * 60_000)).toBe("3 min");
  });
});
