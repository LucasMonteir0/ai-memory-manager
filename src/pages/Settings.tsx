import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ArchiveRestore, BookOpen, Download, FileSearch, Moon, Play, Power, RefreshCw, Rocket, Square, Sun, Trash2, Wrench } from "lucide-react";
import { api, errMsg, isDemo, newOpId } from "@/lib/api";
import type { CmdOutput, ReleaseView, SetupPlanView } from "@/lib/types";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Kv, Pre, Section } from "@/components/ui";
import { detectionTone } from "@/components/StatusBadge";
import { usePlanner } from "@/components/Planner";
import { OpConsole } from "@/components/OpConsole";
import { fmtBytes, fmtDate } from "@/lib/utils";

function BinaryCard() {
  const qc = useQueryClient();
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const d = det.data;
  const pick = async () => {
    const f = await openDialog({ multiple: false, directory: false, filters: [{ name: "ai-memory.exe", extensions: ["exe"] }] });
    if (typeof f !== "string") return;
    try {
      toast.success(await api.setManualBinary(f));
      qc.invalidateQueries();
    } catch (e) {
      toast.error(errMsg(e));
    }
  };
  return (
    <Card>
      <CardHeader>
        <CardTitle>ai-memory</CardTitle>
        {d && <Badge tone={detectionTone[d.state]}>{d.stateLabel}</Badge>}
      </CardHeader>
      <CardBody className="space-y-1">
        <Kv k="Binário" v={d?.binary ?? "não encontrado"} mono />
        <Kv k="Versão" v={d?.version ? `${d.version} (${d.versionSupport === "supported" ? "suportada" : d.versionSupport === "untested" ? "não testada" : "não suportada"})` : "—"} />
        <Kv k="Pasta de dados" v={d?.dataDir ?? "—"} mono />
        <Kv k="Configuração" v={d?.config.exists ? d.config.configPath : "ausente (setup não executado)"} mono />
        <Kv k="Servidor" v={d?.serverUrl ?? "—"} mono />
        <Kv k="Origem da URL" v={d?.config.serverUrl ? "server_url do config.toml" : d?.config.bind ? "bind do config.toml" : "padrão"} />
        <Kv k="Subcomandos detectados" v={d?.commands.length ? `${d.commands.length}` : "—"} />
        {d?.candidates.length ? (
          <details className="pt-1">
            <summary className="cursor-pointer text-xs text-muted">Candidatos encontrados</summary>
            <ul className="mt-1 space-y-0.5 font-mono text-[11px]">
              {d.candidates.map((c) => (
                <li key={c.path}>
                  [{c.source}] {c.path} — {c.valid ? `v${c.version}` : c.error}
                </li>
              ))}
            </ul>
          </details>
        ) : null}
        <div className="flex flex-wrap gap-2 pt-2">
          <Button size="sm" variant="outline" onClick={pick}>
            <FileSearch /> Localizar manualmente…
          </Button>
          <Button size="sm" variant="ghost" onClick={() => det.refetch()} loading={det.isFetching}>
            <RefreshCw /> Detectar novamente
          </Button>
          {d?.dataDir && (
            <Button size="sm" variant="ghost" onClick={() => openPath(d.dataDir).catch((e) => toast.error(errMsg(e)))}>
              Abrir pasta de dados
            </Button>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

function ServerCard() {
  const qc = useQueryClient();
  const planner = usePlanner();
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const [busy, setBusy] = React.useState<"start" | "stop" | null>(null);
  const [confirmStop, setConfirmStop] = React.useState(false);
  const d = det.data;
  const online = d?.health?.reachable;
  const act = async (which: "start" | "stop") => {
    setBusy(which);
    try {
      toast.success(which === "start" ? await api.serverStart() : await api.serverStop());
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setBusy(null);
      setConfirmStop(false);
      qc.invalidateQueries({ queryKey: ["detect"] });
    }
  };
  if (!d?.binary || !d.config.exists) return null;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Servidor global</CardTitle>
        <Badge tone={online ? "ok" : "warn"}>{online ? "online" : "offline"}</Badge>
      </CardHeader>
      <CardBody className="space-y-2">
        <Kv k="Gerenciado por" v={d.service?.managedBy ?? "—"} />
        <Kv k="Supervisor (service-host)" v={d.service?.supervisorPid ? `PID ${d.service.supervisorPid} ${d.service.supervisorAlive ? "ativo" : "inativo"}` : "—"} />
        <Kv k="Servidor (serve)" v={d.service?.serverPid ? `PID ${d.service.serverPid} ${d.service.serverAlive ? "ativo" : "inativo"}` : "—"} />
        <Kv k="Iniciar com Windows (HKCU Run)" v={d.service?.runValuePresent ? "ativo" : "desativado"} />
        <div className="flex flex-wrap gap-2 pt-1">
          <Button size="sm" onClick={() => act("start")} disabled={online || !!busy} loading={busy === "start"}>
            <Play /> Iniciar
          </Button>
          <Button size="sm" variant="outline" onClick={() => setConfirmStop(true)} disabled={!online || !!busy}>
            <Square /> Parar
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => planner.start(() => api.planAutostart(!d.service?.runValuePresent))}
          >
            <Power /> {d.service?.runValuePresent ? "Desativar início com Windows" : "Ativar início com Windows"}
          </Button>
        </div>
        {confirmStop && (
          <Alert tone="warn" title="Parar o servidor?">
            <p>
              Parar o servidor <b>não</b> remove hooks nem MCP. Os agentes continuam gerando eventos, que ficam no spool local até o servidor voltar; não há garantia
              de que a captura foi interrompida. O MCP ficará indisponível. A parada usa o mecanismo do service-host (arquivo <code>service-host.stop</code>);
              nenhum processo é encerrado à força.
            </p>
            <div className="mt-2 flex gap-2">
              <Button size="sm" variant="destructive" onClick={() => act("stop")} loading={busy === "stop"}>
                Parar servidor
              </Button>
              <Button size="sm" variant="outline" onClick={() => setConfirmStop(false)}>
                Cancelar
              </Button>
            </div>
          </Alert>
        )}
      </CardBody>
    </Card>
  );
}

function InstallCard() {
  const qc = useQueryClient();
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const [rel, setRel] = React.useState<ReleaseView | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [addPath, setAddPath] = React.useState(true);
  const [steps, setSteps] = React.useState<string[] | null>(null);
  const [setup, setSetup] = React.useState<SetupPlanView | null>(null);
  const [selAgents, setSelAgents] = React.useState<string[]>([]);
  const [opId, setOpId] = React.useState<string | null>(null);
  const [out, setOut] = React.useState<CmdOutput | null>(null);
  const d = det.data;
  const loadRel = async () => {
    setLoading(true);
    try {
      setRel(await api.aiMemoryRelease());
    } catch (e) {
      toast.error("Sem acesso à release oficial", { description: errMsg(e) });
    } finally {
      setLoading(false);
    }
  };
  const install = async () => {
    setBusy(true);
    try {
      const r = await api.installAiMemory(addPath);
      setSteps(r.steps);
      toast.success(`ai-memory ${r.version} instalado`);
      qc.invalidateQueries();
    } catch (e) {
      toast.error("Instalação falhou", { description: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };
  const loadSetup = async () => {
    try {
      const v = await api.setupPlan();
      setSetup(v);
      if (v.parsed.kind === "ok") setSelAgents(v.parsed.data.agents.map((a) => a.agent));
    } catch (e) {
      toast.error(errMsg(e));
    }
  };
  const runSetup = async () => {
    const id = newOpId();
    setOpId(id);
    setBusy(true);
    try {
      const o = await api.setupApply(selAgents, id);
      setOut(o);
      o.exitCode === 0 ? toast.success("Setup concluído") : toast.error("Setup terminou com erro");
      qc.invalidateQueries();
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setBusy(false);
    }
  };

  if (!d) return null;
  if (d.state === "not_installed" || d.state === "invalid") {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Instalar ai-memory</CardTitle>
          <Badge tone="warn">não instalado</Badge>
        </CardHeader>
        <CardBody className="space-y-3">
          <p className="text-[13px]">
            Baixa somente da release oficial no GitHub, verifica o SHA-256 publicado antes de extrair e instala em pasta do usuário (sem admin). Não executa scripts
            remotos.
          </p>
          {!rel ? (
            <Button variant="outline" onClick={loadRel} loading={loading}>
              <Download /> Consultar release oficial
            </Button>
          ) : (
            <div className="space-y-2">
              <Kv k="Fonte" v={`github.com/${rel.release.repo}`} />
              <Kv k="Editor" v={rel.release.publisher} />
              <Kv k="Versão" v={rel.release.tag} />
              <Kv k="Pacote" v={`${rel.release.assetName ?? "—"} (${fmtBytes(rel.release.assetSize)})`} />
              <Kv k="Checksum" v={rel.release.sha256Url ? "arquivo .sha256 publicado" : "AUSENTE — instalação será recusada"} />
              <Kv k="Destino" v={rel.targetDir} mono />
              <label className="flex items-center gap-2 text-[13px]">
                <input type="checkbox" checked={addPath} onChange={(e) => setAddPath(e.target.checked)} /> Adicionar ao PATH do usuário
              </label>
              <Alert tone="warn">O suporte do ai-memory a Windows nativo é marcado como experimental e os binários não são assinados.</Alert>
              <Button onClick={install} loading={busy} disabled={!rel.release.sha256Url}>
                <Download /> Baixar, verificar e instalar
              </Button>
            </div>
          )}
          {steps && <Pre>{steps.join("\n")}</Pre>}
        </CardBody>
      </Card>
    );
  }
  if (d.state === "no_setup") {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Inicializar ai-memory (setup)</CardTitle>
          <Badge tone="warn">sem setup</Badge>
        </CardHeader>
        <CardBody className="space-y-3">
          <p className="text-[13px]">Executado uma única vez por computador (nunca por repositório). Cria configuração, token e o início com o Windows.</p>
          {!setup ? (
            <Button variant="outline" onClick={loadSetup}>
              Ver plano do setup
            </Button>
          ) : setup.parsed.kind === "ok" ? (
            <div className="space-y-2">
              <Kv k="Pasta de dados" v={setup.parsed.data.dataDir ?? "—"} mono />
              <Kv k="Serviço" v={`${setup.parsed.data.serviceAction ?? "—"} · ${setup.parsed.data.serviceManager ?? ""}`} />
              <div className="text-xs font-semibold text-muted">Agentes detectados (marque os que deseja integrar)</div>
              {setup.parsed.data.agents.map((a) => (
                <label key={a.agent} className="flex items-center gap-2 text-[13px]">
                  <input
                    type="checkbox"
                    checked={selAgents.includes(a.agent)}
                    onChange={(e) => setSelAgents((s) => (e.target.checked ? [...s, a.agent] : s.filter((x) => x !== a.agent)))}
                  />
                  {a.agent} <span className="text-xs text-muted">({a.wiring})</span>
                </label>
              ))}
              <Button onClick={runSetup} loading={busy}>
                <Rocket /> Executar setup
              </Button>
            </div>
          ) : (
            <Alert tone="warn" title="Plano não interpretado">
              <Pre>{setup.output.stdout + setup.output.stderr}</Pre>
            </Alert>
          )}
          <OpConsole opId={opId} />
          {out && <Pre>{`código ${out.exitCode}\n${out.stdout}`}</Pre>}
        </CardBody>
      </Card>
    );
  }
  return null;
}

function UpgradeCard() {
  const qc = useQueryClient();
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const [rel, setRel] = React.useState<ReleaseView | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [confirm, setConfirm] = React.useState(false);
  const [opId, setOpId] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [out, setOut] = React.useState<CmdOutput | null>(null);
  if (!det.data?.binary) return null;
  const check = async () => {
    setLoading(true);
    try {
      setRel(await api.aiMemoryRelease());
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setLoading(false);
    }
  };
  const run = async () => {
    const id = newOpId();
    setOpId(id);
    setBusy(true);
    try {
      const o = await api.upgradeAiMemory(id);
      setOut(o);
      qc.invalidateQueries();
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
        <CardTitle>Atualizar ai-memory</CardTitle>
        {rel && <Badge tone={rel.updateAvailable ? "warn" : "ok"}>{rel.updateAvailable ? `nova: ${rel.release.tag}` : "em dia"}</Badge>}
      </CardHeader>
      <CardBody className="space-y-2">
        <p className="text-[13px] text-muted">Usa o mecanismo oficial (<code>ai-memory upgrade</code>, verifica .sha256). Nunca automático.</p>
        <Button size="sm" variant="outline" onClick={check} loading={loading}>
          <RefreshCw /> Verificar release
        </Button>
        {rel && (
          <>
            <Kv k="Instalada" v={rel.installedVersion ?? "—"} />
            <Kv k="Mais recente" v={`${rel.release.tag} (${fmtDate(rel.release.publishedAt)})`} />
            {rel.release.changelog && (
              <details>
                <summary className="cursor-pointer text-xs text-muted">Mudanças</summary>
                <Pre className="mt-1">{rel.release.changelog}</Pre>
              </details>
            )}
            {rel.updateAvailable &&
              (!confirm ? (
                <Button size="sm" onClick={() => setConfirm(true)}>
                  Atualizar…
                </Button>
              ) : (
                <Alert tone="warn" title="Confirmar atualização">
                  Um backup das configurações dos agentes e do config.toml será criado. Depois, recomenda-se reaplicar hooks (Agentes → Reparar), pois os comandos de
                  hook podem mudar.
                  <div className="mt-2 flex gap-2">
                    <Button size="sm" onClick={run} loading={busy}>
                      Atualizar agora
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => setConfirm(false)}>
                      Cancelar
                    </Button>
                  </div>
                </Alert>
              ))}
          </>
        )}
        <OpConsole opId={opId} />
        {out && <Pre>{out.stdout + "\n" + out.stderr}</Pre>}
      </CardBody>
    </Card>
  );
}

function AppUpdateCard() {
  const env = useQuery({ queryKey: ["env"], queryFn: api.envInfo });
  const [upd, setUpd] = React.useState<Update | null | undefined>(undefined);
  const [checking, setChecking] = React.useState(false);
  const [progress, setProgress] = React.useState<string | null>(null);
  const portable = env.data?.installKind === "portable";
  const doCheck = async () => {
    setChecking(true);
    try {
      setUpd(await check());
    } catch (e) {
      toast.error("Não foi possível verificar atualizações", { description: errMsg(e) });
      setUpd(undefined);
    } finally {
      setChecking(false);
    }
  };
  const install = async () => {
    if (!upd) return;
    let total = 0;
    let got = 0;
    try {
      await upd.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        if (ev.event === "Progress") {
          got += ev.data.chunkLength;
          setProgress(total ? `${Math.round((got / total) * 100)}%` : fmtBytes(got));
        }
        if (ev.event === "Finished") setProgress("instalando…");
      });
      await relaunch();
    } catch (e) {
      toast.error("Falha na atualização", { description: errMsg(e) });
      setProgress(null);
    }
  };
  return (
    <Card>
      <CardHeader>
        <CardTitle>Atualizar este aplicativo</CardTitle>
        <Badge>v{env.data?.appVersion}</Badge>
      </CardHeader>
      <CardBody className="space-y-2">
        <p className="text-[13px] text-muted">
          Releases assinadas em github.com/lucasmonteir0/ai-memory-manager. Separado da atualização do ai-memory. Instala somente após seu clique.
        </p>
        {portable && <Alert tone="info">Versão portátil: a atualização automática só funciona na versão instalada. Use o link para baixar manualmente.</Alert>}
        <div className="flex flex-wrap gap-2">
          {!portable && !isDemo && (
            <Button size="sm" variant="outline" onClick={doCheck} loading={checking}>
              <RefreshCw /> Procurar atualizações
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => openUrl("https://github.com/lucasmonteir0/ai-memory-manager/releases")}>
            Página de releases
          </Button>
        </div>
        {upd === null && <Alert tone="ok">Você já tem a versão mais recente.</Alert>}
        {upd && (
          <Alert tone="info" title={`Versão ${upd.version} disponível`}>
            {upd.body && <Pre className="mt-1">{upd.body}</Pre>}
            <Button size="sm" className="mt-2" onClick={install} loading={!!progress}>
              <Download /> Atualizar e reiniciar {progress && `(${progress})`}
            </Button>
          </Alert>
        )}
      </CardBody>
    </Card>
  );
}

function BackupsCard() {
  const planner = usePlanner();
  const b = useQuery({ queryKey: ["backups"], queryFn: api.listBackups });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Backups das configurações alteradas</CardTitle>
        <Badge>{b.data?.length ?? 0}</Badge>
      </CardHeader>
      <CardBody className="space-y-1">
        {b.data?.length === 0 && <p className="text-[13px] text-muted">Nenhum backup ainda. Cada alteração feita pela GUI cria um.</p>}
        <ul className="scroll-thin max-h-72 space-y-1 overflow-auto">
          {b.data?.map((s) => (
            <li key={s.id} className="flex items-center justify-between gap-2 rounded-md border border-border px-3 py-1.5 text-[13px]">
              <div className="min-w-0">
                <div className="truncate font-medium">{s.reason}</div>
                <div className="truncate text-xs text-muted">
                  {fmtDate(s.createdAt)} · {s.files.length} arquivo(s){s.registry.length ? ` · ${s.registry.length} registro(s)` : ""}
                </div>
              </div>
              <Button size="sm" variant="ghost" onClick={() => planner.start(() => api.planRestoreBackup(s.id))}>
                <ArchiveRestore /> Restaurar…
              </Button>
            </li>
          ))}
        </ul>
        <p className="pt-1 text-xs text-muted">
          Os backups ficam em <code>%APPDATA%\AI Memory Manager\backups</code> (somente seu usuário). Podem conter configurações dos agentes; tokens gravados pela
          GUI no registro nunca são copiados para backups.
        </p>
      </CardBody>
    </Card>
  );
}

function AdvancedCard() {
  const planner = usePlanner();
  return (
    <Card>
      <CardHeader>
        <CardTitle>Avançado</CardTitle>
        <Wrench className="size-4 text-muted" />
      </CardHeader>
      <CardBody className="space-y-2">
        <Alert tone="warn">
          O desinstalador oficial (<code>ai-memory uninstall --only …</code>) não filtra por agente: remove de <b>todos</b> os agentes detectados. Para um agente só,
          use os botões em Agentes. Nunca usamos <code>--purge-data</code>.
        </Alert>
        <div className="flex flex-wrap gap-2">
          {["hooks", "mcp", "skills", "instructions"].map((o) => (
            <Button key={o} size="sm" variant="outline" onClick={() => planner.start(() => api.planUninstall(o))}>
              <Trash2 /> Remover {o} de todos…
            </Button>
          ))}
        </div>
      </CardBody>
    </Card>
  );
}

export function Settings({ theme, setTheme }: { theme: "light" | "dark"; setTheme: (t: "light" | "dark") => void }) {
  const env = useQuery({ queryKey: ["env"], queryFn: api.envInfo });
  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-xl font-semibold">Configurações</h1>
        <p className="text-[13px] text-muted">Binário, servidor, inicialização, atualização e backups.</p>
      </div>
      <InstallCard />
      <div className="grid gap-4 xl:grid-cols-2">
        <BinaryCard />
        <ServerCard />
        <UpgradeCard />
        <AppUpdateCard />
      </div>
      <BackupsCard />
      <Section title="Preferências e ambiente">
        <div className="grid gap-4 xl:grid-cols-2">
          <Card>
            <CardBody className="space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-[13px]">Tema</span>
                <Button size="sm" variant="outline" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
                  {theme === "dark" ? <Sun /> : <Moon />} {theme === "dark" ? "Claro" : "Escuro"}
                </Button>
              </div>
              <Kv k="Idioma" v="Português (Brasil)" />
              <Kv k="Sistema" v={`${env.data?.os ?? "—"} ${env.data?.arch ?? ""} · Windows nativo (sem WSL)`} />
              <Kv k="Dados da GUI" v={env.data?.appDir ?? "—"} mono />
              <Kv k="CLAUDE_CONFIG_DIR" v={env.data?.claudeConfigDirEnv ?? "(não definido)"} mono />
              <Kv k="CODEX_HOME" v={env.data?.codexHomeEnv ?? "(não definido)"} mono />
            </CardBody>
          </Card>
          <Card>
            <CardBody className="space-y-1 text-[13px]">
              <div className="mb-1 font-semibold">Documentação oficial do ai-memory</div>
              {[
                ["Instalação", "docs/install.md"],
                ["Windows nativo (experimental)", "docs/windows.md"],
                ["MCP e hooks por cliente", "docs/mcp-install.md"],
                ["Marker .ai-memory.toml", "docs/marker-file.md"],
                ["Matriz de suporte", "docs/support-matrix.md"],
                ["Segurança", "docs/security.md"],
              ].map(([t, p]) => (
                <button key={p} className="flex items-center gap-1.5 text-accent hover:underline" onClick={() => openUrl(`https://github.com/akitaonrails/ai-memory/blob/main/${p}`)}>
                  <BookOpen className="size-3.5" /> {t}
                </button>
              ))}
            </CardBody>
          </Card>
        </div>
      </Section>
      <AdvancedCard />
    </div>
  );
}
