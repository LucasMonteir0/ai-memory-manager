import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Ban, FlaskConical, FolderOpen, FolderPlus, History, Pencil, Plus, Square, Stethoscope, Tag, Trash2 } from "lucide-react";
import { api, errMsg, isDemo, newOpId } from "@/lib/api";
import type { BackfillRun, Doctor, DoctorRun, FolderInspection, Project } from "@/lib/types";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Dialog, DialogContent, Empty, Input, Kv, Label, Pre, Section } from "@/components/ui";
import { IntegrationBadge } from "@/components/StatusBadge";
import { usePlanner } from "@/components/Planner";
import { OpConsole } from "@/components/OpConsole";
import { agentLabel, backfillOutcome, cn, fmtDate, fmtNum } from "@/lib/utils";

function AddProjectDialog({ open, onOpenChange, initialPath }: { open: boolean; onOpenChange: (o: boolean) => void; initialPath?: string }) {
  const qc = useQueryClient();
  const [path, setPath] = React.useState("");
  const [name, setName] = React.useState("");
  const [insp, setInsp] = React.useState<FolderInspection | null>(null);
  const [err, setErr] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const inspect = React.useCallback(async (p: string) => {
    setErr(null);
    setInsp(null);
    if (!p.trim()) return;
    try {
      const i = await api.inspectFolder(p);
      setInsp(i);
      setName((n) => n || i.resolvedPath.split(/[\\/]/).pop() || "");
    } catch (e) {
      setErr(errMsg(e));
    }
  }, []);

  React.useEffect(() => {
    if (open) {
      setPath(initialPath ?? "");
      setName("");
      setInsp(null);
      setErr(null);
      if (initialPath) inspect(initialPath);
    }
  }, [open, initialPath, inspect]);

  const browse = async () => {
    const sel = await openDialog({ directory: true, multiple: false, title: "Selecione a pasta do projeto" });
    if (typeof sel === "string") {
      setPath(sel);
      inspect(sel);
    }
  };

  const add = async () => {
    if (!insp) return;
    setBusy(true);
    try {
      await api.addProject(insp.resolvedPath, name);
      toast.success("Projeto adicionado ao catálogo", { description: "Nada foi alterado na pasta. Rode o doctor para ver a identidade." });
      qc.invalidateQueries({ queryKey: ["projects"] });
      onOpenChange(false);
    } catch (e) {
      setErr(errMsg(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title="Adicionar projeto" description="Registra a pasta apenas no catálogo da GUI. Não reinstala o servidor nem duplica hooks globais.">
        <div className="space-y-3">
          <div className="flex gap-2">
            <Input
              placeholder="F:\repos\meu-projeto"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onBlur={() => inspect(path)}
              onKeyDown={(e) => e.key === "Enter" && inspect(path)}
              aria-label="Caminho da pasta"
            />
            <Button variant="outline" onClick={browse}>
              <FolderOpen /> Procurar…
            </Button>
          </div>
          <p className="text-xs text-muted">Dica: você também pode arrastar uma pasta para a janela.</p>
          {err && <Alert tone="err">{err}</Alert>}
          {insp && (
            <Card>
              <CardBody className="space-y-1">
                <Kv k="Caminho resolvido" v={insp.resolvedPath} mono />
                <Kv k="Git" v={insp.isGit ? (insp.gitRootDiffers ? `sim (raiz: ${insp.gitRoot})` : "sim (raiz)") : "não"} />
                {insp.subRepos.length > 0 && <Kv k="Sub-repositórios" v={insp.subRepos.map((s) => s.name).join(", ")} />}
                <Kv
                  k="Marker .ai-memory.toml"
                  v={insp.marker ? `${insp.marker.inSelectedFolder ? "nesta pasta" : "em pasta ancestral"} (${insp.marker.workspace ?? "?"}/${insp.marker.project ?? "auto"})` : "ausente"}
                />
                <Kv k="Identidade provável" v={`${insp.suggestedWorkspace}/${insp.suggestedProject}`} />
                <p className="pt-1 text-xs text-muted">
                  A identidade definitiva é a que o <code>ai-memory doctor</code> resolver (pode vir do remote git, do marker ou do nome da pasta).
                </p>
                {insp.warnings.map((w, i) => (
                  <Alert key={i} tone="warn" className="mt-2">
                    {w}
                  </Alert>
                ))}
              </CardBody>
            </Card>
          )}
          <div>
            <Label htmlFor="pname">Rótulo local (só na GUI)</Label>
            <Input id="pname" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Cancelar
            </Button>
            <Button onClick={add} disabled={!insp} loading={busy}>
              <Plus /> Adicionar
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function MarkerBox({ project, doctor, insp }: { project: Project; doctor?: Doctor; insp?: FolderInspection }) {
  const planner = usePlanner();
  const [ws, setWs] = React.useState("default");
  const [pr, setPr] = React.useState("");
  React.useEffect(() => {
    setWs(doctor?.workspace ?? insp?.suggestedWorkspace ?? "default");
    setPr(doctor?.project ?? insp?.suggestedProject ?? "");
  }, [doctor, insp]);
  const marker = insp?.marker;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Identidade</CardTitle>
        <Badge tone={marker?.inSelectedFolder ? "ok" : "muted"}>{marker ? (marker.inSelectedFolder ? "Pinada por marker" : "Marker ancestral") : "Sem marker (fallback)"}</Badge>
      </CardHeader>
      <CardBody className="space-y-2">
        <Kv k="Resolvida pelo doctor" v={doctor ? `${doctor.workspace}/${doctor.project}` : "rode o doctor"} />
        <Kv k="Origem" v={doctor?.scopeSource ?? "—"} />
        <Kv k="No servidor" v={doctor?.serverProjectStatus ?? "—"} />
        {marker ? (
          <Pre>{`${marker.path}\nworkspace = ${marker.workspace ?? "(não definido)"}\nproject = ${marker.project ?? "(automático)"}${marker.hasIgnorePaths ? "\n[capture] ignore_paths definido" : ""}`}</Pre>
        ) : (
          <div className="space-y-2 rounded-md border border-border p-3">
            <p className="text-[13px] text-muted">
              Opcional: criar <code>.ai-memory.toml</code> estabiliza a identidade para todas as sessões nesta pasta. O arquivo pode entrar no git.
            </p>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label>workspace</Label>
                <Input value={ws} onChange={(e) => setWs(e.target.value)} />
              </div>
              <div>
                <Label>project</Label>
                <Input value={pr} onChange={(e) => setPr(e.target.value)} />
              </div>
            </div>
            {doctor && pr && pr !== doctor.project && (
              <Alert tone="warn">O nome difere do atual ({doctor.project}); novas sessões irão para outra identidade.</Alert>
            )}
            <Button size="sm" variant="outline" onClick={() => planner.start(() => api.planMarker(project.id, ws, pr))}>
              <Tag /> Prévia do marker
            </Button>
          </div>
        )}
      </CardBody>
    </Card>
  );
}

function BackfillBox({ project }: { project: Project }) {
  const qc = useQueryClient();
  const [max, setMax] = React.useState(25);
  const [force, setForce] = React.useState(false);
  const [dry, setDry] = React.useState<BackfillRun | null>(null);
  const [real, setReal] = React.useState<BackfillRun | null>(null);
  const [busy, setBusy] = React.useState<"dry" | "run" | null>(null);
  const [opId, setOpId] = React.useState<string | null>(null);
  const dryKey = `${force}|${max}`;
  const [dryFor, setDryFor] = React.useState<string | null>(null);

  const simulate = async () => {
    setBusy("dry");
    setReal(null);
    try {
      const r = await api.backfillDryRun(project.id, force, max);
      setDry(r);
      setDryFor(r.output.exitCode === 0 && r.parsed.kind === "ok" ? dryKey : null);
    } catch (e) {
      toast.error("Simulação falhou", { description: errMsg(e) });
    } finally {
      setBusy(null);
    }
  };
  const run = async () => {
    const id = newOpId();
    setOpId(id);
    setBusy("run");
    try {
      const r = await api.backfillRun(project.id, force, max, id);
      setReal(r);
      setDryFor(null);
      const o = backfillOutcome(r, max);
      if (o.tone === "ok") toast.success(o.title);
      else if (o.tone === "err") toast.error(o.title);
      else toast.warning(o.title);
      qc.invalidateQueries();
    } catch (e) {
      toast.error("Importação falhou", { description: errMsg(e) });
    } finally {
      setBusy(null);
    }
  };

  const d = dry?.parsed.kind === "ok" ? dry.parsed.data : null;
  const r = real?.parsed.kind === "ok" ? real.parsed.data : null;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Importar histórico (backfill)</CardTitle>
        <Badge tone="info">sempre simula antes</Badge>
      </CardHeader>
      <CardBody className="space-y-3">
        <div className="flex flex-wrap items-end gap-3">
          <div className="w-32">
            <Label htmlFor="max">Máx. sessões</Label>
            <Input id="max" type="number" min={1} max={10000} value={max} onChange={(e) => setMax(Math.max(1, Number(e.target.value) || 1))} />
          </div>
          <label className="flex items-center gap-2 pb-2 text-[13px]">
            <input type="checkbox" checked={force} onChange={(e) => setForce(e.target.checked)} />
            Forçar (--force)
          </label>
          <Button variant="outline" onClick={simulate} loading={busy === "dry"} disabled={!!busy}>
            <FlaskConical /> Simular (dry-run)
          </Button>
          <Button onClick={run} disabled={dryFor !== dryKey || !!busy} loading={busy === "run"} title={dryFor !== dryKey ? "Simule primeiro com os mesmos parâmetros" : undefined}>
            <History /> Importar
          </Button>
          {busy === "run" && opId && (
            <Button variant="destructive" onClick={() => api.cancelOperation(opId)}>
              <Square /> Cancelar
            </Button>
          )}
        </div>
        {d && (
          <div className="rounded-md border border-border p-3 text-[13px]">
            <div className="mb-1 font-semibold">Simulação: {d.workspace}/{d.project}</div>
            {d.skippedNonEmpty ? (
              <Alert tone="warn">
                O ai-memory pulou a importação porque o armazenamento já tem sessões ({`skipped_non_empty`}). Para importar este projeto mesmo assim, marque “Forçar”
                e simule de novo. A deduplicação é feita pelo próprio ai-memory; quantidades podem diferir das sessões locais.
              </Alert>
            ) : (
              <div className="grid grid-cols-2 gap-x-6 md:grid-cols-4">
                <Kv k="Selecionadas" v={fmtNum(d.selected)} />
                <Kv k="Puladas (limite)" v={fmtNum(d.skippedForCap)} />
                <Kv k="Falhas" v={fmtNum(d.failedSessions)} />
                <Kv k="Não admitidas" v={d.skippedNotAdmitted ? "sim" : "não"} />
              </div>
            )}
          </div>
        )}
        {dry?.parsed.kind === "raw" && (
          <Alert tone="warn" title={`Simulação não interpretada: ${dry.parsed.reason}`}>
            <Pre>{dry.output.stdout + dry.output.stderr}</Pre>
          </Alert>
        )}
        <OpConsole opId={busy === "run" || real ? opId : null} />
        {r && (
          <div className="grid grid-cols-2 gap-x-6 rounded-md border border-border p-3 md:grid-cols-4">
            <Kv k="Sessões importadas" v={fmtNum(r.importedSessions)} />
            <Kv k="Eventos importados" v={fmtNum(r.importedEvents)} />
            <Kv k="Falhas" v={fmtNum(r.failedSessions)} />
            <Kv k="Puladas (limite)" v={fmtNum(r.skippedForCap)} />
          </div>
        )}
        {real &&
          (() => {
            const o = backfillOutcome(real, max);
            return (
              <Alert tone={o.tone} title={o.title}>
                {o.notes.length > 0 && (
                  <ul className="mt-1 list-disc space-y-0.5 pl-5">
                    {o.notes.map((n, i) => (
                      <li key={i}>{n}</li>
                    ))}
                  </ul>
                )}
              </Alert>
            );
          })()}
        <p className="text-xs text-muted">
          Backfill importa histórico; ele <b>não</b> prova que hooks de sessões novas estão funcionando — use “Verificar captura” em Agentes.
        </p>
      </CardBody>
    </Card>
  );
}

function ProjectDetail({ project }: { project: Project }) {
  const qc = useQueryClient();
  const [run, setRun] = React.useState<DoctorRun | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [renaming, setRenaming] = React.useState(false);
  const [newName, setNewName] = React.useState(project.displayName);
  const [confirmRemove, setConfirmRemove] = React.useState(false);
  const cached = useQuery({ queryKey: ["cachedDoctor", project.id], queryFn: () => api.cachedDoctor(project.id) });
  const insp = useQuery({ queryKey: ["inspect", project.absolutePath], queryFn: () => api.inspectFolder(project.absolutePath), retry: false });
  const agents = useQuery({ queryKey: ["agents", project.id], queryFn: () => api.agentsView(project.id, false) });

  React.useEffect(() => {
    setRun(null);
    setNewName(project.displayName);
  }, [project.id, project.displayName]);

  const doctor: Doctor | undefined = run?.parsed.kind === "ok" ? run.parsed.data : cached.data?.[0];
  const doctorAt = run?.ranAt ?? cached.data?.[1] ?? project.lastDoctorAt;

  const diagnose = async () => {
    setBusy(true);
    try {
      const r = await api.runDoctor(project.id);
      setRun(r);
      qc.invalidateQueries({ queryKey: ["projects"] });
      qc.invalidateQueries({ queryKey: ["agents"] });
      qc.invalidateQueries({ queryKey: ["cachedDoctor", project.id] });
      if (r.parsed.kind === "raw") toast.warning("Saída do doctor não interpretada — veja o log.");
    } catch (e) {
      toast.error("doctor falhou", { description: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };

  const agentRows = ["claude-code", "codex", "cursor"].map((id) => ({
    id,
    row: doctor?.rows.find((r) => r.agent === id),
    rep: agents.data?.agents.find((a) => a.id === id),
  }));
  const extraRows = doctor?.rows.filter((r) => !["claude-code", "codex", "cursor"].includes(r.agent)) ?? [];

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <div className="min-w-0">
            {renaming ? (
              <div className="flex gap-2">
                <Input value={newName} onChange={(e) => setNewName(e.target.value)} className="h-8" />
                <Button
                  size="sm"
                  onClick={async () => {
                    await api.renameProject(project.id, newName).catch((e) => toast.error(errMsg(e)));
                    setRenaming(false);
                    qc.invalidateQueries({ queryKey: ["projects"] });
                  }}
                >
                  Salvar
                </Button>
              </div>
            ) : (
              <h2 className="truncate text-lg font-semibold">{project.displayName}</h2>
            )}
            <div className="truncate font-mono text-xs text-muted" title={project.absolutePath}>
              {project.absolutePath}
            </div>
            <div className="mt-1 flex flex-wrap gap-2 text-xs">
              <Badge>workspace: {doctor?.workspace ?? project.lastWorkspace ?? "?"}</Badge>
              <Badge>project: {doctor?.project ?? project.lastProject ?? "?"}</Badge>
            </div>
          </div>
          <div className="flex shrink-0 flex-wrap justify-end gap-2">
            <Button onClick={diagnose} loading={busy}>
              <Stethoscope /> Diagnosticar
            </Button>
            <Button variant="outline" onClick={() => openPath(project.absolutePath).catch((e) => toast.error(errMsg(e)))}>
              <FolderOpen /> Abrir pasta
            </Button>
            <Button variant="ghost" size="icon" title="Renomear rótulo" onClick={() => setRenaming((r) => !r)}>
              <Pencil />
            </Button>
            <Button variant="ghost" size="icon" title="Remover da lista" onClick={() => setConfirmRemove(true)}>
              <Trash2 />
            </Button>
          </div>
        </CardHeader>
        <CardBody>
          <div className="mb-2 flex flex-wrap gap-4 text-[13px]">
            <span>
              Identidade: <b>{doctor?.markerStatus === "present" ? "pinada" : doctor ? "fallback" : "?"}</b>
            </span>
            <span>
              Servidor: <b>{doctor ? (doctor.serverUnreachable ? "inacessível" : "online") : "?"}</b>
            </span>
            <span className="text-muted">
              Último doctor: {fmtDate(doctorAt)} {doctor && "(teste de configuração, não fim-a-fim)"}
            </span>
          </div>
          {insp.error && <Alert tone="err">Pasta inacessível: {errMsg(insp.error)}</Alert>}
          <div className="overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead className="text-left text-xs text-muted">
                <tr className="border-b border-border">
                  <th className="py-1.5 font-medium">Agente</th>
                  <th className="font-medium">Sessões locais</th>
                  <th className="font-medium">Capturadas</th>
                  <th className="font-medium">MCP</th>
                  <th className="font-medium">Hooks</th>
                </tr>
              </thead>
              <tbody>
                {agentRows.map(({ id, row, rep }) => (
                  <tr key={id} className="border-b border-border/60">
                    <td className="py-1.5">{agentLabel[id]}</td>
                    <td className="tabular-nums">{row ? fmtNum(row.localTotal) : doctor ? "0" : "—"}</td>
                    <td className={cn("tabular-nums", row?.uncaptured && "text-warn")}>{row ? fmtNum(row.captured) : doctor ? "0" : "—"}</td>
                    <td>{rep ? <IntegrationBadge status={rep.mcp.status} /> : "—"}</td>
                    <td>{rep ? <IntegrationBadge status={rep.hooks.status} /> : "—"}</td>
                  </tr>
                ))}
                {extraRows.map((r) => (
                  <tr key={r.agent} className="border-b border-border/60 text-muted">
                    <td className="py-1.5">{agentLabel[r.agent] ?? r.agent}</td>
                    <td>{fmtNum(r.localTotal)}</td>
                    <td>{fmtNum(r.captured)}</td>
                    <td colSpan={2}>—</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-xs text-muted">
            “Locais” são sessões encontradas nos arquivos do agente; “capturadas” são as que chegaram ao ai-memory. Ausência de captura ≠ ausência de sessão.
          </p>
          {doctor?.warnings.map((w, i) => (
            <Alert key={i} tone="warn" className="mt-2">
              {w}
            </Alert>
          ))}
          {doctor?.nativeMemory.map((m, i) => (
            <p key={i} className="mt-1 text-xs text-muted">
              Memória nativa do {agentLabel[m.agent] ?? m.agent}: {m.fileCount} arquivo(s) ({m.status}).
            </p>
          ))}
          {run && (
            <details className="mt-3">
              <summary className="cursor-pointer text-xs font-semibold text-muted">
                Saída original do doctor (cwd: {run.cwd}) {run.parsed.kind === "raw" && <Badge tone="warn">Não interpretado</Badge>}
              </summary>
              <Pre className="mt-2">{`$ ${run.output.command}\n${run.output.stdout}\n${run.output.stderr}`}</Pre>
            </details>
          )}
        </CardBody>
      </Card>

      <div className="grid gap-4 xl:grid-cols-2">
        <MarkerBox project={project} doctor={doctor} insp={insp.data} />
        <Card>
          <CardHeader>
            <CardTitle>Captura deste projeto</CardTitle>
            <Badge tone="muted">
              <Ban className="size-3" /> Não disponível
            </Badge>
          </CardHeader>
          <CardBody className="space-y-2 text-[13px]">
            <p>
              <b>Desativar a captura só deste projeto não está disponível com a integração global atual.</b> Os hooks dos agentes são globais e disparam
              em qualquer pasta.
            </p>
            <p className="text-muted">
              <code>[capture] ignore_paths</code> apenas filtra eventos ligados a caminhos/arquivos — não desliga conversas inteiras, por isso não é oferecido
              como “desativar”. Para parar a captura, desative os hooks do agente (afeta todos os projetos) em Agentes.
            </p>
            <p className="text-muted">Remover da lista, desativar captura e apagar memória são coisas diferentes: esta GUI nunca apaga memórias.</p>
          </CardBody>
        </Card>
      </div>

      <BackfillBox project={project} />

      <Dialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <DialogContent title="Remover da lista?" description="Remove apenas do catálogo desta GUI.">
          <p className="text-[13px]">
            As memórias, o marker <code>.ai-memory.toml</code>, hooks e MCP <b>não</b> são alterados. Você pode adicionar a pasta novamente a qualquer momento.
          </p>
          <div className="mt-4 flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConfirmRemove(false)}>
              Cancelar
            </Button>
            <Button
              variant="destructive"
              onClick={async () => {
                try {
                  toast.success(await api.removeProject(project.id));
                } catch (e) {
                  toast.error(errMsg(e));
                }
                setConfirmRemove(false);
                qc.invalidateQueries({ queryKey: ["projects"] });
              }}
            >
              Remover da lista
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export function Projects() {
  const list = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });
  const [sel, setSel] = React.useState<string | null>(null);
  const [adding, setAdding] = React.useState(false);
  const [dropPath, setDropPath] = React.useState<string | undefined>();
  const [dragOver, setDragOver] = React.useState(false);

  React.useEffect(() => {
    if (isDemo) return;
    let un: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        if (ev.payload.type === "over" || ev.payload.type === "enter") setDragOver(true);
        else if (ev.payload.type === "leave") setDragOver(false);
        else if (ev.payload.type === "drop") {
          setDragOver(false);
          const p = ev.payload.paths[0];
          if (p) {
            setDropPath(p);
            setAdding(true);
          }
        }
      })
      .then((u) => (un = u));
    return () => un?.();
  }, []);

  React.useEffect(() => {
    if (!sel && list.data?.length) setSel(list.data[0].id);
  }, [list.data, sel]);
  const current = list.data?.find((p) => p.id === sel);

  return (
    <div className={cn("space-y-4", dragOver && "outline-2 outline-dashed outline-accent")}>
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">Projetos</h1>
          <p className="text-[13px] text-muted">Registrar um projeto não reinstala o servidor nem duplica hooks globais.</p>
        </div>
        <Button
          onClick={() => {
            setDropPath(undefined);
            setAdding(true);
          }}
        >
          <FolderPlus /> Adicionar projeto
        </Button>
      </div>
      {list.data?.length === 0 ? (
        <Empty icon={<FolderPlus className="size-8" />} title="Nenhum projeto no catálogo">
          Adicione uma pasta pelo seletor, colando o caminho ou arrastando-a para a janela.
        </Empty>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[260px_1fr]">
          <Section title="Catálogo">
            <nav className="space-y-1" aria-label="Projetos">
              {list.data?.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setSel(p.id)}
                  className={cn("w-full rounded-md border px-3 py-2 text-left", sel === p.id ? "border-accent bg-panel" : "border-transparent hover:bg-panel")}
                >
                  <div className="truncate text-[13px] font-medium">{p.displayName}</div>
                  <div className="truncate font-mono text-[11px] text-muted">{p.absolutePath}</div>
                  <div className="mt-0.5 text-[11px] text-muted">{p.lastProject ? `${p.lastWorkspace}/${p.lastProject}` : "doctor pendente"}</div>
                </button>
              ))}
            </nav>
          </Section>
          <div>{current ? <ProjectDetail key={current.id} project={current} /> : null}</div>
        </div>
      )}
      <AddProjectDialog open={adding} onOpenChange={setAdding} initialPath={dropPath} />
    </div>
  );
}
