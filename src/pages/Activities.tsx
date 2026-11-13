import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ClipboardCopy, Droplets, Square } from "lucide-react";
import { api, errMsg, newOpId } from "@/lib/api";
import type { AppOperation, CmdOutput } from "@/lib/types";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Empty, Input, Label, Pre } from "@/components/ui";
import { OpConsole } from "@/components/OpConsole";
import { agentLabel, cn, fmtAge, fmtDate, fmtNum } from "@/lib/utils";

function SpoolCard() {
  const qc = useQueryClient();
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const s = det.data?.status?.kind === "ok" ? det.data.status.data : undefined;
  const [opId, setOpId] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [out, setOut] = React.useState<CmdOutput | null>(null);
  const [confirm, setConfirm] = React.useState(false);
  const run = async () => {
    const id = newOpId();
    setOpId(id);
    setBusy(true);
    setOut(null);
    try {
      const o = await api.recoverRun(id);
      setOut(o);
      if (o.exitCode === 0) toast.success("Recuperação concluída.");
      else toast.error("Recuperação terminou com erro.");
      qc.invalidateQueries({ queryKey: ["detect"] });
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  return (
    <Card>
      <CardHeader>
        <CardTitle>Spool de eventos</CardTitle>
        <Badge tone={(s?.spoolOldestAgeMs ?? 0) > 600_000 ? "warn" : "muted"}>
          {fmtNum(s?.spoolPending)} pendente(s) · mais antigo {fmtAge(s?.spoolOldestAgeMs)}
        </Badge>
      </CardHeader>
      <CardBody className="space-y-2">
        <p className="text-[13px] text-muted">
          Eventos dos hooks ficam no spool local quando o servidor está indisponível. Alguns pendentes por poucos segundos é normal.
          {det.data && !det.data.hasRecover && det.data.hasHookDrain && " Esta versão usa hook-drain."}
        </p>
        {!confirm ? (
          <Button variant="outline" onClick={() => setConfirm(true)} disabled={busy || (!det.data?.hasRecover && !det.data?.hasHookDrain)}>
            <Droplets /> Drenar spool / recuperar
          </Button>
        ) : (
          <Alert tone="info" title="Executar ai-memory recover?">
            <p>Drena o spool local, reprocessa transcrições de execuções degradadas e finaliza importações pendentes. É seguro executar novamente.</p>
            <div className="mt-2 flex gap-2">
              <Button size="sm" onClick={run} loading={busy}>
                Confirmar
              </Button>
              {busy && opId ? (
                <Button size="sm" variant="destructive" onClick={() => api.cancelOperation(opId)}>
                  <Square /> Cancelar
                </Button>
              ) : (
                <Button size="sm" variant="outline" onClick={() => setConfirm(false)}>
                  Voltar
                </Button>
              )}
            </div>
          </Alert>
        )}
        <OpConsole opId={opId} />
        {out && <Pre>{out.stdout || out.stderr}</Pre>}
      </CardBody>
    </Card>
  );
}

export function Activities() {
  const ops = useQuery({ queryKey: ["ops", "all"], queryFn: () => api.listOperations(1000) });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });
  const [kind, setKind] = React.useState("");
  const [agent, setAgent] = React.useState("");
  const [project, setProject] = React.useState("");
  const [date, setDate] = React.useState("");
  const [sel, setSel] = React.useState<AppOperation | null>(null);
  const detail = useQuery({ queryKey: ["opDetail", sel?.id], queryFn: () => api.operationDetail(sel!.id), enabled: !!sel });
  const filtered = (ops.data ?? []).filter(
    (o) =>
      (!kind || o.kind === kind) &&
      (!agent || o.agentId === agent) &&
      (!project || o.projectId === project) &&
      (!date || o.startedAt.startsWith(date)),
  );
  const copyReport = async () => {
    try {
      const r = await api.sanitizedReport();
      await navigator.clipboard.writeText(r);
      toast.success("Relatório sanitizado copiado", { description: "Tokens, cabeçalhos e segredos foram redigidos." });
    } catch (e) {
      toast.error(errMsg(e));
    }
  };
  const projName = (id?: string | null) => projects.data?.find((p) => p.id === id)?.displayName ?? (id ? "?" : "");
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">Atividades</h1>
          <p className="text-[13px] text-muted">Log local de operações (sanitizado). Leituras, testes e alterações com backup associado.</p>
        </div>
        <Button variant="outline" onClick={copyReport}>
          <ClipboardCopy /> Copiar relatório sanitizado
        </Button>
      </div>
      <SpoolCard />
      <Card>
        <CardHeader>
          <CardTitle>Operações</CardTitle>
          <span className="text-xs text-muted">{filtered.length} de {ops.data?.length ?? 0}</span>
        </CardHeader>
        <CardBody className="space-y-3">
          <div className="grid gap-2 md:grid-cols-4">
            <div>
              <Label>Data</Label>
              <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
            </div>
            <div>
              <Label>Tipo</Label>
              <select className="h-9 w-full rounded-md border border-border bg-panel px-2 text-sm" value={kind} onChange={(e) => setKind(e.target.value)}>
                <option value="">Todos</option>
                <option value="leitura">Leitura</option>
                <option value="teste">Teste</option>
                <option value="alteracao">Alteração</option>
              </select>
            </div>
            <div>
              <Label>Agente</Label>
              <select className="h-9 w-full rounded-md border border-border bg-panel px-2 text-sm" value={agent} onChange={(e) => setAgent(e.target.value)}>
                <option value="">Todos</option>
                {Object.entries(agentLabel).map(([k, v]) => (
                  <option key={k} value={k}>
                    {v}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <Label>Projeto</Label>
              <select className="h-9 w-full rounded-md border border-border bg-panel px-2 text-sm" value={project} onChange={(e) => setProject(e.target.value)}>
                <option value="">Todos</option>
                {projects.data?.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.displayName}
                  </option>
                ))}
              </select>
            </div>
          </div>
          {filtered.length === 0 ? (
            <Empty title="Nenhuma operação">As ações executadas pela GUI aparecem aqui.</Empty>
          ) : (
            <div className="grid gap-3 lg:grid-cols-[1fr_1fr]">
              <ul className="scroll-thin max-h-[480px] space-y-1 overflow-auto">
                {filtered.map((o) => (
                  <li key={o.id}>
                    <button
                      onClick={() => setSel(o)}
                      className={cn("w-full rounded-md border px-3 py-2 text-left text-[13px]", sel?.id === o.id ? "border-accent" : "border-border hover:bg-panel-2")}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium">{o.action}</span>
                        <Badge tone={o.success ? "ok" : o.success === false ? "err" : "muted"}>{o.success ? "ok" : o.success === false ? "falhou" : "—"}</Badge>
                      </div>
                      <div className="mt-0.5 flex flex-wrap gap-x-3 text-xs text-muted">
                        <span>{fmtDate(o.startedAt)}</span>
                        <span>{o.kind}</span>
                        {o.agentId && <span>{agentLabel[o.agentId] ?? o.agentId}</span>}
                        {o.projectId && <span>{projName(o.projectId)}</span>}
                        {o.backupId && <span>backup {o.backupId}</span>}
                      </div>
                      {o.summary && <div className="mt-0.5 truncate text-xs">{o.summary}</div>}
                    </button>
                  </li>
                ))}
              </ul>
              <div>{sel ? <Pre className="max-h-[480px]">{detail.data || sel.summary || "(sem detalhes)"}</Pre> : <p className="text-[13px] text-muted">Selecione uma operação.</p>}</div>
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
