import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { AlertTriangle, CheckCircle2, FileDiff as FileDiffIcon, RotateCcw, ShieldCheck, Terminal, XCircle } from "lucide-react";
import { api, errMsg } from "@/lib/api";
import type { ApplyResult, ChangePlan, FileDiff } from "@/lib/types";
import { Alert, Badge, Button, Dialog, DialogContent, Pre } from "./ui";
import { cn } from "@/lib/utils";

type Ctx = { start: (make: () => Promise<ChangePlan>, onDone?: (r: ApplyResult) => void) => Promise<void> };
const PlannerCtx = React.createContext<Ctx | null>(null);

export function usePlanner() {
  const c = React.useContext(PlannerCtx);
  if (!c) throw new Error("PlannerProvider ausente");
  return c;
}

export function DiffView({ d }: { d: FileDiff }) {
  return (
    <div className="rounded-md border border-border">
      <div className="flex items-center justify-between gap-2 border-b border-border bg-panel-2 px-3 py-1.5 text-xs">
        <span className="truncate font-mono" title={d.path}>
          {d.path}
        </span>
        <span className="flex shrink-0 gap-2">
          {!d.existedBefore && <Badge tone="info">novo</Badge>}
          <span className="text-ok">+{d.added}</span>
          <span className="text-err">−{d.removed}</span>
        </span>
      </div>
      {d.changed ? (
        <pre className="scroll-thin max-h-72 overflow-auto p-2 font-mono text-[11.5px] leading-relaxed">
          {d.unified.split("\n").map((l, i) => (
            <div
              key={i}
              className={cn(
                l.startsWith("+") && !l.startsWith("+++") && "bg-ok/10 text-ok",
                l.startsWith("-") && !l.startsWith("---") && "bg-err/10 text-err",
                l.startsWith("@@") && "text-info",
              )}
            >
              {l || " "}
            </div>
          ))}
        </pre>
      ) : (
        <div className="px-3 py-2 text-xs text-muted">Sem alterações.</div>
      )}
    </div>
  );
}

export function PlannerProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient();
  const [plan, setPlan] = React.useState<ChangePlan | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [applying, setApplying] = React.useState(false);
  const [result, setResult] = React.useState<ApplyResult | null>(null);
  const [ack, setAck] = React.useState(false);
  const [open, setOpen] = React.useState(false);
  const doneRef = React.useRef<((r: ApplyResult) => void) | undefined>(undefined);

  const start = React.useCallback(async (make: () => Promise<ChangePlan>, onDone?: (r: ApplyResult) => void) => {
    setLoading(true);
    setResult(null);
    setAck(false);
    try {
      const p = await make();
      doneRef.current = onDone;
      setPlan(p);
      setOpen(true);
    } catch (e) {
      toast.error("Não foi possível preparar a prévia", { description: errMsg(e) });
    } finally {
      setLoading(false);
    }
  }, []);

  const close = () => {
    if (plan && !result) api.discardPlan(plan.id).catch(() => {});
    setOpen(false);
    setPlan(null);
    setResult(null);
  };

  const apply = async () => {
    if (!plan) return;
    setApplying(true);
    try {
      const r = await api.applyPlan(plan.id);
      setResult(r);
      if (r.success) toast.success(r.summary);
      else toast.error(r.summary);
      doneRef.current?.(r);
      qc.invalidateQueries();
    } catch (e) {
      toast.error("Falha ao aplicar", { description: errMsg(e) });
    } finally {
      setApplying(false);
    }
  };

  return (
    <PlannerCtx.Provider value={{ start }}>
      {children}
      {loading && (
        <div className="fixed right-4 bottom-4 z-50 rounded-md border border-border bg-panel px-3 py-2 text-[13px] shadow">Preparando prévia…</div>
      )}
      <Dialog open={open} onOpenChange={(o) => !o && close()}>
        {plan && (
          <DialogContent wide title={plan.title} description={result ? "Resultado" : "Prévia da alteração — nada foi modificado ainda"}>
            {!result ? (
              <div className="space-y-4">
                <p className="text-[13px]">{plan.explanation}</p>
                {plan.destructive && (
                  <Alert tone="err" title="Operação de alcance amplo">
                    Leia a prévia com atenção. Um backup será criado antes.
                  </Alert>
                )}
                {plan.warnings.map((w, i) => (
                  <Alert key={i} tone="warn">
                    <span className="inline-flex gap-1.5">
                      <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
                      {w}
                    </span>
                  </Alert>
                ))}
                {plan.files.length > 0 && (
                  <div>
                    <div className="mb-1 text-xs font-semibold text-muted">Arquivos/chaves afetados (backup automático)</div>
                    <ul className="space-y-0.5 font-mono text-xs">
                      {plan.files.map((f) => (
                        <li key={f} className="truncate" title={f}>
                          {f}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                {plan.commands.length > 0 && (
                  <div>
                    <div className="mb-1 flex items-center gap-1 text-xs font-semibold text-muted">
                      <Terminal className="size-3.5" /> Comando que será executado (sem shell)
                    </div>
                    <Pre>{plan.commands.join("\n")}</Pre>
                  </div>
                )}
                {plan.diffs.length > 0 && (
                  <div className="space-y-2">
                    <div className="flex items-center gap-1 text-xs font-semibold text-muted">
                      <FileDiffIcon className="size-3.5" /> Diff previsto
                    </div>
                    {plan.diffs.map((d) => (
                      <DiffView key={d.path} d={d} />
                    ))}
                  </div>
                )}
                {plan.dryRunOutput && (
                  <details>
                    <summary className="cursor-pointer text-xs font-semibold text-muted">Saída do dry-run (proposta, não evidência de instalação ativa)</summary>
                    <Pre className="mt-2">{plan.dryRunOutput}</Pre>
                  </details>
                )}
                {plan.afterSteps.length > 0 && (
                  <div className="text-[13px]">
                    <div className="mb-1 text-xs font-semibold text-muted">Depois de aplicar</div>
                    <ul className="list-disc space-y-0.5 pl-5">
                      {plan.afterSteps.map((s, i) => (
                        <li key={i}>{s}</li>
                      ))}
                    </ul>
                  </div>
                )}
                {plan.destructive && (
                  <label className="flex items-center gap-2 text-[13px]">
                    <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
                    Entendi que isso afeta todos os agentes listados.
                  </label>
                )}
                <div className="flex justify-end gap-2 border-t border-border pt-3">
                  <Button variant="outline" onClick={close}>
                    Cancelar
                  </Button>
                  <Button variant={plan.destructive ? "destructive" : "default"} onClick={apply} loading={applying} disabled={plan.destructive && !ack}>
                    <ShieldCheck /> Confirmar e aplicar
                  </Button>
                </div>
              </div>
            ) : (
              <div className="space-y-4">
                <Alert tone={result.success ? "ok" : "err"} title={result.summary}>
                  <span className="inline-flex items-center gap-1.5">
                    {result.success ? <CheckCircle2 className="size-3.5" /> : result.rolledBack ? <RotateCcw className="size-3.5" /> : <XCircle className="size-3.5" />}
                    {result.backupId ? `Backup: ${result.backupId}` : "Sem backup"}
                  </span>
                </Alert>
                <div>
                  <div className="mb-1 text-xs font-semibold text-muted">Verificação</div>
                  <Pre>{result.verification}</Pre>
                </div>
                {result.diffs.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-xs font-semibold text-muted">Alterações aplicadas</div>
                    {result.diffs.map((d) => (
                      <DiffView key={d.path} d={d} />
                    ))}
                  </div>
                )}
                {result.output && (
                  <details>
                    <summary className="cursor-pointer text-xs font-semibold text-muted">Saída do comando (código {String(result.output.exitCode)})</summary>
                    <Pre className="mt-2">{`$ ${result.output.command}\n${result.output.stdout}\n${result.output.stderr}`}</Pre>
                  </details>
                )}
                {result.afterSteps.length > 0 && (
                  <Alert tone="info" title="Próximos passos (manuais)">
                    <ul className="list-disc pl-5">
                      {result.afterSteps.map((s, i) => (
                        <li key={i}>{s}</li>
                      ))}
                    </ul>
                  </Alert>
                )}
                <div className="flex justify-end border-t border-border pt-3">
                  <Button onClick={close}>Fechar</Button>
                </div>
              </div>
            )}
          </DialogContent>
        )}
      </Dialog>
    </PlannerCtx.Provider>
  );
}
