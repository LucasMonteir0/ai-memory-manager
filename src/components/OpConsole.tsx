import * as React from "react";
import { listenOpLines } from "@/lib/api";
import type { OpLine } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Mostra stdout/stderr em tempo real de uma operação longa (backfill, setup, upgrade). */
export function OpConsole({ opId, className }: { opId: string | null; className?: string }) {
  const [lines, setLines] = React.useState<OpLine[]>([]);
  const ref = React.useRef<HTMLPreElement>(null);
  React.useEffect(() => {
    setLines([]);
    if (!opId) return;
    let un: (() => void) | undefined;
    listenOpLines((l) => {
      if (l.opId === opId) setLines((prev) => [...prev.slice(-2000), l]);
    }).then((u) => (un = u));
    return () => un?.();
  }, [opId]);
  React.useEffect(() => {
    ref.current?.scrollTo({ top: ref.current.scrollHeight });
  }, [lines]);
  if (!opId) return null;
  return (
    <pre
      ref={ref}
      aria-live="polite"
      className={cn("scroll-thin h-48 overflow-auto rounded-md border border-border bg-panel-2 p-2 font-mono text-[11.5px] leading-relaxed", className)}
    >
      {lines.length === 0 ? <span className="text-muted">Aguardando saída…</span> : null}
      {lines.map((l, i) => (
        <div key={i} className={cn(l.stream === "stderr" && "text-muted")}>
          {l.line}
        </div>
      ))}
    </pre>
  );
}
