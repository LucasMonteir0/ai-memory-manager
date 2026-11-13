import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { BookOpen, Cable, CheckCircle2, FileCog, KeyRound, Plug, PlugZap, RefreshCw, ScanSearch, ScrollText, Unplug, Wrench } from "lucide-react";
import { api, errMsg } from "@/lib/api";
import type { AgentReport, CaptureEvidence, Component, McpTests } from "@/lib/types";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Kv, Label, Pre } from "@/components/ui";
import { IntegrationBadge } from "@/components/StatusBadge";
import { usePlanner } from "@/components/Planner";
import { fmtDate } from "@/lib/utils";
import type { Page } from "@/App";

function ComponentBlock({
  title,
  c,
  agent,
  kind,
  onLog,
}: {
  title: string;
  c: Component;
  agent: AgentReport;
  kind: "mcp" | "hooks";
  onLog: () => void;
}) {
  const planner = usePlanner();
  const configure = () => planner.start(() => api.planAgentChange(agent.id, kind, "enable"));
  const disable = () => planner.start(() => api.planAgentChange(agent.id, kind, "disable"));
  const isSet = ["configured", "verified", "partial", "untrusted", "error"].includes(c.status);
  return (
    <div className="space-y-2 rounded-md border border-border p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-[13px] font-semibold">
          {kind === "mcp" ? <Plug className="size-4" /> : <ScanSearch className="size-4" />}
          {title}
        </div>
        <div className="flex gap-1.5">
          {c.experimental && <Badge tone="warn">Experimental</Badge>}
          {c.source && <Badge>{c.source}</Badge>}
          <IntegrationBadge status={c.status} />
        </div>
      </div>
      <p className="text-[13px]">{c.detail}</p>
      {c.experimental && <Alert tone="warn">{c.experimental}</Alert>}
      {c.evidence.length > 0 && (
        <ul className="list-disc space-y-0.5 pl-5 text-xs text-muted">
          {c.evidence.map((e, i) => (
            <li key={i}>{e}</li>
          ))}
        </ul>
      )}
      {c.configPath && (
        <div className="truncate font-mono text-[11px] text-muted" title={c.configPath}>
          {c.configPath}
        </div>
      )}
      {c.blockedReason && c.status !== "unsupported" && <Alert tone="muted">{c.blockedReason}</Alert>}
      {c.status !== "unsupported" && (
        <div className="flex flex-wrap gap-2 pt-1">
          {c.canConfigure && (
            <Button size="sm" variant={isSet ? "outline" : "default"} onClick={configure}>
              {isSet ? <Wrench /> : <PlugZap />} {isSet ? "Reparar" : kind === "mcp" ? "Configurar MCP" : "Configurar hooks"}
            </Button>
          )}
          {c.canDisable && (
            <Button size="sm" variant="outline" onClick={disable}>
              <Unplug /> {kind === "mcp" ? "Desconectar MCP" : "Desativar captura"}
            </Button>
          )}
          {c.configPath && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() =>
                openPath(c.configPath!).catch(() => revealItemInDir(c.configPath!).catch((e) => toast.error("Não foi possível abrir", { description: errMsg(e) })))
              }
            >
              <FileCog /> Abrir configuração
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={onLog}>
            <ScrollText /> Ver log
          </Button>
        </div>
      )}
    </div>
  );
}

function CaptureTest({ agent }: { agent: AgentReport }) {
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });
  const [pid, setPid] = React.useState<string>("");
  const [ev, setEv] = React.useState<CaptureEvidence | null>(null);
  const [busy, setBusy] = React.useState(false);
  const qc = useQueryClient();
  React.useEffect(() => {
    if (!pid && projects.data?.length) setPid(projects.data[0].id);
  }, [projects.data, pid]);
  if (!agent.hooksAgentId || agent.hooks.status === "unsupported" || agent.hooks.status === "not_configured" || agent.hooks.status === "not_detected") return null;
  const step = async (fn: () => Promise<CaptureEvidence>) => {
    setBusy(true);
    try {
      const r = await fn();
      setEv(r);
      if (r.verified) {
        toast.success(r.message);
        qc.invalidateQueries({ queryKey: ["agents"] });
      }
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-2 rounded-md border border-dashed border-border p-3">
      <div className="text-[13px] font-semibold">Verificar captura (teste fim-a-fim com sessão nova)</div>
      {projects.data?.length ? (
        <div className="flex flex-wrap items-end gap-2">
          <div>
            <Label htmlFor={`p-${agent.id}`}>Projeto</Label>
            <select
              id={`p-${agent.id}`}
              className="h-8 rounded-md border border-border bg-panel px-2 text-[13px]"
              value={pid}
              onChange={(e) => setPid(e.target.value)}
            >
              {projects.data.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.displayName}
                </option>
              ))}
            </select>
          </div>
          <Button size="sm" variant="outline" loading={busy && !ev} onClick={() => step(() => api.captureTestStart(agent.id, pid))}>
            1. Registrar estado atual
          </Button>
          <Button size="sm" disabled={!ev} loading={busy && !!ev} onClick={() => step(() => api.captureTestCheck(agent.id))}>
            2. Conferir
          </Button>
        </div>
      ) : (
        <p className="text-xs text-muted">Adicione um projeto para testar.</p>
      )}
      {ev && (
        <Alert tone={ev.verified ? "ok" : "info"}>
          {ev.message} {ev.checkedAt && `(${fmtDate(ev.checkedAt)})`}
        </Alert>
      )}
    </div>
  );
}

function AgentCard({ a, go }: { a: AgentReport; go: (p: Page) => void }) {
  const planner = usePlanner();
  const codexAuthGap = a.id === "codex" && a.mcp.status === "error" && a.mcp.detail.includes("variável");
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{a.displayName}</CardTitle>
          <p className="text-xs text-muted">{a.description}</p>
        </div>
        <Badge tone={a.app.installed ? "ok" : a.app.installed === false ? "muted" : "warn"}>
          {a.app.installed ? "Detectado" : a.app.installed === false ? "Não detectado" : "Não verificável"}
        </Badge>
      </CardHeader>
      <CardBody className="space-y-3">
        <div className="grid gap-x-6 md:grid-cols-2">
          <Kv k="Aplicativo" v={a.app.source ?? "—"} />
          <Kv k="Versão" v={a.app.version ?? "—"} />
          <Kv k="ID MCP / hooks" v={`${a.mcpClientId ?? "—"} / ${a.hooksAgentId ?? "—"}`} mono />
          <Kv k="Escopo" v={a.hooksScope} />
        </div>
        <div className="grid gap-3 xl:grid-cols-2">
          <ComponentBlock title="MCP (consulta de memória)" c={a.mcp} agent={a} kind="mcp" onLog={() => go("activities")} />
          <ComponentBlock title="Hooks (captura de eventos)" c={a.hooks} agent={a} kind="hooks" onLog={() => go("activities")} />
        </div>
        {codexAuthGap && (
          <Alert tone="err" title="Autenticação do MCP do Codex">
            <div className="space-y-2">
              <p>O Codex lê o token de uma variável de ambiente que não existe. Opções:</p>
              <div className="flex flex-wrap gap-2">
                <Button size="sm" onClick={() => planner.start(() => api.planCodexBridge())}>
                  <Cable /> Usar ponte local (recomendado)
                </Button>
                <Button size="sm" variant="outline" onClick={() => planner.start(() => api.planCodexTokenFix())}>
                  <KeyRound /> Criar variável com o token
                </Button>
              </div>
              <p className="text-xs">
                A ponte local (<code>ai-memory.exe mcp-bridge</code>) lê o token sozinha do diretório de dados, sem gravar segredo. A variável grava o token no
                registro do usuário.
              </p>
            </div>
          </Alert>
        )}
        {a.cursorHookConflict > 0 && (
          <Alert tone="err" title="Estes hooks bloqueiam os prompts no Cursor">
            <div className="space-y-2">
              <p>
                O Cursor também carrega os hooks do Claude Code, mas ignora os argumentos (<code>args</code>). O <code>ai-memory.exe</code> roda sem
                argumentos, imprime a ajuda, e o Cursor recusa o prompt com <code>ERROR_HOOKS_BLOCKED</code>. {a.cursorHookConflict} hook(s) afetado(s).
              </p>
              <Button size="sm" onClick={() => planner.start(() => api.planCursorCompatHooks())}>
                <Wrench /> Converter para formato compatível
              </Button>
            </div>
          </Alert>
        )}
        {a.untrustedEvents.length > 0 && (
          <Alert tone="warn" title="Ação manual necessária no Codex Desktop">
            Abra o Codex Desktop e aprove os hooks pendentes ({a.untrustedEvents.join(", ")}). Não há API oficial para autorizar pela GUI; o comando{" "}
            <code>/hooks</code> pode não existir na interface desktop.
          </Alert>
        )}
        <CaptureTest agent={a} />
        {a.notes.length > 0 && (
          <ul className="list-disc space-y-0.5 pl-5 text-xs text-muted">
            {a.notes.map((n, i) => (
              <li key={i}>{n}</li>
            ))}
          </ul>
        )}
        <div className="flex justify-end">
          <Button size="sm" variant="ghost" onClick={() => openUrl(a.docsUrl)}>
            <BookOpen /> Documentação oficial
          </Button>
        </div>
      </CardBody>
    </Card>
  );
}

export function Agents({ go }: { go: (p: Page) => void }) {
  const q = useQuery({ queryKey: ["agents", null], queryFn: () => api.agentsView(null, false) });
  const [mcp, setMcp] = React.useState<McpTests | null>(null);
  const [testing, setTesting] = React.useState(false);
  const refresh = async () => {
    await api.agentsView(null, true);
    q.refetch();
  };
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">Agentes</h1>
          <p className="text-[13px] text-muted">MCP dá consulta de memória; hooks dão captura. Um pode funcionar sem o outro. Não requer os CLIs `claude`/`codex`.</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            loading={testing}
            onClick={async () => {
              setTesting(true);
              try {
                setMcp(await api.testMcp());
              } catch (e) {
                toast.error(errMsg(e));
              } finally {
                setTesting(false);
              }
            }}
          >
            <CheckCircle2 /> Testar servidor MCP
          </Button>
          <Button variant="outline" onClick={refresh} loading={q.isFetching}>
            <RefreshCw /> Detectar novamente
          </Button>
        </div>
      </div>
      {q.data?.doctorSource ? (
        <p className="text-xs text-muted">Dados de captura/trust do {q.data.doctorSource}.</p>
      ) : (
        <Alert tone="info">Rode o doctor em um projeto para ver contagens de captura e o estado de autorização dos hooks do Codex.</Alert>
      )}
      {mcp && (
        <Card>
          <CardHeader>
            <CardTitle>Teste do servidor MCP (nível: servidor)</CardTitle>
            <span className="text-xs text-muted">{fmtDate(mcp.withToken.testedAt)}</span>
          </CardHeader>
          <CardBody className="space-y-2">
            <Alert tone={mcp.withToken.ok ? "ok" : "err"}>{mcp.interpretation}</Alert>
            <div className="grid gap-x-6 md:grid-cols-2">
              <Kv k="Com token" v={mcp.withToken.ok ? `OK · ${mcp.withToken.toolCount} ferramentas` : mcp.withToken.error ?? "falhou"} />
              <Kv k="Sem token" v={mcp.withoutToken.ok ? "OK" : `HTTP ${mcp.withoutToken.httpStatus ?? "—"}`} />
              <Kv k="Servidor" v={`${mcp.withToken.serverName ?? "—"} ${mcp.withToken.serverVersion ?? ""}`} />
              <Kv k="Endpoint" v={mcp.withToken.endpoint} mono />
            </div>
            <p className="text-xs text-muted">
              Este teste prova que o servidor e o token funcionam via MCP (initialize + tools/list). Ele <b>não</b> prova que cada aplicativo carregou sua
              configuração — confirme abrindo uma sessão no agente e pedindo, por exemplo, “use memory_status”.
            </p>
            {mcp.withToken.toolNames.length > 0 && (
              <details>
                <summary className="cursor-pointer text-xs text-muted">Ferramentas expostas</summary>
                <Pre className="mt-1">{mcp.withToken.toolNames.join("\n")}</Pre>
              </details>
            )}
          </CardBody>
        </Card>
      )}
      {q.isLoading && <div className="text-[13px] text-muted">Detectando aplicativos e configurações…</div>}
      {q.error && <Alert tone="err">{errMsg(q.error)}</Alert>}
      <div className="space-y-4">
        {q.data?.agents.map((a) => (
          <AgentCard key={a.id} a={a} go={go} />
        ))}
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Outros agentes</CardTitle>
        </CardHeader>
        <CardBody className="text-[13px] text-muted">
          O ai-memory suporta OpenCode, Gemini CLI, Kiro, Cline, Qwen Code, Copilot CLI, Zed e outros. Esta versão do gerenciador cobre os três alvos
          prioritários; demais integrações ficam para a fase 2 e podem ser feitas pelo próprio <code>ai-memory setup</code>.
        </CardBody>
      </Card>
    </div>
  );
}
