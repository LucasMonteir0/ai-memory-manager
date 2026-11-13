import { useQuery } from "@tanstack/react-query";
import { Activity, AlertTriangle, Database, FolderPlus, Globe, HeartPulse, History, Plug, RefreshCw } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "@/lib/api";
import type { Detection, Status } from "@/lib/types";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Kv, Pre, Section } from "@/components/ui";
import { detectionTone, IntegrationBadge } from "@/components/StatusBadge";
import { fmtAge, fmtBytes, fmtDate, fmtNum } from "@/lib/utils";
import type { Page } from "@/App";

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <Card>
      <CardBody>
        <div className="text-xs text-muted">{label}</div>
        <div className="mt-1 text-2xl font-semibold tabular-nums">{value}</div>
        {hint && <div className="mt-0.5 text-xs text-muted">{hint}</div>}
      </CardBody>
    </Card>
  );
}

export function alertsFrom(det?: Detection, agents?: Awaited<ReturnType<typeof api.agentsView>>): { tone: "warn" | "err" | "info"; text: string; go?: Page }[] {
  const out: { tone: "warn" | "err" | "info"; text: string; go?: Page }[] = [];
  if (!det) return out;
  if (det.state === "not_installed") out.push({ tone: "warn", text: "ai-memory não está instalado.", go: "settings" });
  if (det.state === "server_stopped") out.push({ tone: "warn", text: "Servidor parado: hooks continuam gravando no spool local até ele voltar.", go: "settings" });
  if (det.state === "auth_error") out.push({ tone: "err", text: "Erro de autenticação com o servidor.", go: "settings" });
  if (det.state === "unknown") out.push({ tone: "warn", text: "Saída do status não interpretada — veja a saída original abaixo." });
  if (det.versionSupport === "untested") out.push({ tone: "warn", text: `Versão ${det.version} não testada por este gerenciador.` });
  if (det.service?.notes.length) det.service.notes.forEach((n) => out.push({ tone: "warn", text: n, go: "settings" }));
  const s = det.status?.kind === "ok" ? det.status.data : undefined;
  if (s?.spoolPending && (s.spoolOldestAgeMs ?? 0) > 10 * 60_000)
    out.push({ tone: "warn", text: `Spool atrasado: ${s.spoolPending} evento(s), o mais antigo há ${fmtAge(s.spoolOldestAgeMs)}.`, go: "activities" });
  if (s?.ingestDroppedUnauthorized) out.push({ tone: "err", text: `${s.ingestDroppedUnauthorized} evento(s) recusados por autenticação.` });
  for (const a of agents?.agents ?? []) {
    if (a.id === "claude-cowork") continue;
    if (a.mcp.status === "error") out.push({ tone: "err", text: `${a.displayName}: ${a.mcp.detail}`, go: "agents" });
    if (a.hooks.status === "untrusted") out.push({ tone: "warn", text: `${a.displayName}: hooks não autorizados (ação manual no app).`, go: "agents" });
    if (a.hooks.status === "partial") out.push({ tone: "warn", text: `${a.displayName}: hooks parciais ou duplicados.`, go: "agents" });
    if (a.app.installed && a.mcp.status === "not_configured" && a.id !== "claude-desktop")
      out.push({ tone: "info", text: `${a.displayName}: MCP ausente.`, go: "agents" });
  }
  return out;
}

export function Overview({ go }: { go: (p: Page) => void }) {
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const agents = useQuery({ queryKey: ["agents", null], queryFn: () => api.agentsView(null, false), enabled: !!det.data?.binary });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });
  const ops = useQuery({ queryKey: ["ops"], queryFn: () => api.listOperations(6) });
  const d = det.data;
  const s: Status | undefined = d?.status?.kind === "ok" ? d.status.data : undefined;
  const alerts = alertsFrom(d, agents.data);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">Visão geral</h1>
          <p className="text-[13px] text-muted">Instalação global única do ai-memory · memórias organizadas por workspace/projeto.</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => go("projects")}>
            <FolderPlus /> Adicionar projeto
          </Button>
          <Button variant="outline" onClick={() => det.refetch()} loading={det.isFetching}>
            <HeartPulse /> Verificar saúde
          </Button>
          <Button variant="outline" onClick={() => go("memories")}>
            <Database /> Ver memórias
          </Button>
          <Button variant="outline" onClick={() => go("agents")}>
            <Plug /> Corrigir integração
          </Button>
          <Button variant="outline" onClick={() => go("projects")}>
            <History /> Importar histórico
          </Button>
        </div>
      </div>

      {det.isError && <Alert tone="err" title="Falha ao consultar o ai-memory">{String(det.error)}</Alert>}

      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <CardTitle>Servidor</CardTitle>
            {d && <Badge tone={detectionTone[d.state]}>{d.stateLabel}</Badge>}
          </div>
          <span className="text-xs text-muted">verificado {fmtDate(d?.checkedAt)}</span>
        </CardHeader>
        <CardBody className="grid gap-x-8 md:grid-cols-2">
          <div>
            <Kv k="Versão (cliente)" v={d?.version ?? "—"} />
            <Kv k="URL" v={d?.serverUrl ?? "—"} mono />
            <Kv k="Saúde (/healthz)" v={d?.health ? (d.health.reachable ? `HTTP ${d.health.httpStatus} · ${d.health.latencyMs} ms` : `inacessível`) : "—"} />
            <Kv k="Autenticação" v={s?.authPosture ? `${s.authPosture}${d?.tokenAvailable ? " (token encontrado)" : ""}` : "—"} />
          </div>
          <div>
            <Kv k="Gerenciado por" v={d?.service?.managedBy ?? "—"} />
            <Kv k="Iniciar com Windows" v={d?.service ? (d.service.runValuePresent ? "ativo (HKCU Run)" : "desativado") : "—"} />
            <Kv k="Modo de captura" v={s?.captureMode ?? "—"} />
            <Kv k="Banco" v={fmtBytes(s?.databaseBytes)} />
          </div>
        </CardBody>
      </Card>

      {alerts.length > 0 && (
        <Section title="Alertas">
          <div className="space-y-2">
            {alerts.map((a, i) => (
              <Alert key={i} tone={a.tone}>
                <div className="flex items-center justify-between gap-3">
                  <span className="inline-flex gap-1.5">
                    <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
                    {a.text}
                  </span>
                  {a.go && (
                    <Button size="sm" variant="ghost" onClick={() => go(a.go!)}>
                      Abrir
                    </Button>
                  )}
                </div>
              </Alert>
            ))}
          </div>
        </Section>
      )}

      <Section title="Memórias globais">
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <Stat label="Páginas compiladas" value={fmtNum(s?.pages)} />
          <Stat label="Sessões armazenadas" value={fmtNum(s?.sessions)} />
          <Stat label="Observações (eventos)" value={fmtNum(s?.observations)} />
          <Stat label="Spool pendente" value={fmtNum(s?.spoolPending)} hint={s?.spoolPending ? `mais antigo há ${fmtAge(s.spoolOldestAgeMs)}` : "transitório é normal"} />
        </div>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <Stat label="Último evento aceito" value={s?.lastPersistedMs ? new Date(s.lastPersistedMs).toLocaleTimeString("pt-BR") : "—"} hint={s?.lastPersistedMs ? new Date(s.lastPersistedMs).toLocaleDateString("pt-BR") : undefined} />
          <Stat label="Falhas de ingestão" value={fmtNum(s?.ingestFailed)} />
          <Stat label="Índice FTS" value={s?.ftsPagesOk == null ? "—" : s.ftsPagesOk && s.ftsObservationsOk ? "OK" : "Divergente"} />
          <Stat label="Embeddings" value={s?.embeddingStatus ?? "—"} hint="ausência é informativa, não falha" />
        </div>
        {d?.status?.kind === "raw" && (
          <Alert tone="warn" title={`Status não interpretado: ${d.status.reason}`}>
            <Pre className="mt-2">{d.status.raw || d.statusCommand?.stderr}</Pre>
          </Alert>
        )}
      </Section>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Agentes</CardTitle>
            <Button size="sm" variant="ghost" onClick={() => go("agents")}>
              Detalhes
            </Button>
          </CardHeader>
          <CardBody className="space-y-1.5">
            {agents.isLoading && <div className="text-[13px] text-muted">Detectando aplicativos…</div>}
            {agents.data?.agents
              .filter((a) => a.id !== "claude-cowork")
              .map((a) => (
                <div key={a.id} className="flex items-center justify-between gap-2 text-[13px]">
                  <span>{a.displayName}</span>
                  <span className="flex gap-1.5">
                    <span className="text-xs text-muted">MCP</span>
                    <IntegrationBadge status={a.mcp.status} />
                    <span className="text-xs text-muted">Hooks</span>
                    <IntegrationBadge status={a.hooks.status} />
                  </span>
                </div>
              ))}
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Projetos e últimas operações</CardTitle>
            <Button size="sm" variant="ghost" onClick={() => go("activities")}>
              <Activity /> Atividades
            </Button>
          </CardHeader>
          <CardBody>
            <Kv k="Projetos no catálogo" v={String(projects.data?.length ?? 0)} />
            <div className="mt-2 space-y-1">
              {ops.data?.map((o) => (
                <div key={o.id} className="flex items-center justify-between gap-2 text-[13px]">
                  <span className="truncate">{o.action}</span>
                  <span className="flex shrink-0 items-center gap-2 text-xs text-muted">
                    {fmtDate(o.startedAt)}
                    <Badge tone={o.success ? "ok" : o.success === false ? "err" : "muted"}>{o.success ? "ok" : o.success === false ? "falhou" : "—"}</Badge>
                  </span>
                </div>
              ))}
              {ops.data?.length === 0 && <div className="text-[13px] text-muted">Nenhuma operação ainda.</div>}
            </div>
          </CardBody>
        </Card>
      </div>

      {d?.webUrl && (
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => openUrl(d.webUrl)}>
            <Globe /> Abrir UI web do ai-memory
          </Button>
          <Button variant="ghost" size="sm" onClick={() => det.refetch()}>
            <RefreshCw /> Recarregar
          </Button>
        </div>
      )}
    </div>
  );
}
