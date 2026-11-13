import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { BrainCircuit, Globe } from "lucide-react";
import { api } from "@/lib/api";
import { Alert, Badge, Button, Card, CardBody, CardHeader, CardTitle, Kv } from "@/components/ui";
import { fmtBytes, fmtNum } from "@/lib/utils";

export function Memories() {
  const det = useQuery({ queryKey: ["detect"], queryFn: api.detect });
  const s = det.data?.status?.kind === "ok" ? det.data.status.data : undefined;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">Memórias</h1>
          <p className="text-[13px] text-muted">O ai-memory é a fonte da verdade. A GUI não lê nem edita o banco SQLite diretamente.</p>
        </div>
        <Button onClick={() => det.data && openUrl(det.data.webUrl)} disabled={!det.data}>
          <Globe /> Abrir UI web nativa
        </Button>
      </div>
      <Alert tone="info">
        A interface web (<code>{det.data?.webUrl ?? "/web"}</code>) abre no navegador e pede login próprio. O token bearer não é repassado pela GUI. Se o servidor
        não foi iniciado com <code>--enable-web</code>, a página não existirá.
      </Alert>
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Armazenamento</CardTitle>
          </CardHeader>
          <CardBody>
            <Kv k="Páginas compiladas" v={fmtNum(s?.pages)} />
            <Kv k="Sessões armazenadas" v={fmtNum(s?.sessions)} />
            <Kv k="Observações (eventos)" v={fmtNum(s?.observations)} />
            <Kv k="Eventos aceitos (desde o início do servidor)" v={fmtNum(s?.ingestAccepted)} />
            <Kv k="Tamanho do banco" v={fmtBytes(s?.databaseBytes)} />
            <Kv k="Consolidações com falha" v={fmtNum(s?.consolidationFailed)} />
            <p className="mt-2 text-xs text-muted">
              <b>Sessões locais</b> (arquivos do agente) ≠ <b>sessões capturadas</b> (chegaram via hooks) ≠ <b>sessões armazenadas</b> (no banco, incluindo
              backfill) ≠ <b>páginas</b> (conhecimento compilado). Veja os números por projeto em Projetos → Diagnosticar.
            </p>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Modelos opcionais</CardTitle>
            <BrainCircuit className="size-4 text-muted" />
          </CardHeader>
          <CardBody>
            <Kv k="LLM (consolidação)" v={<Badge tone={s?.llmStatus === "disabled" ? "muted" : "info"}>{s?.llmStatus ?? "—"}</Badge>} />
            <Kv k="Embeddings" v={<Badge tone={s?.embeddingStatus === "disabled" ? "muted" : "info"}>{s?.embeddingStatus ?? "—"}</Badge>} />
            <Kv k="Linhas de embedding" v={fmtNum(s?.embeddingRows)} />
            <p className="mt-2 text-xs text-muted">
              Somente leitura nesta versão. Provedores de modelo (embeddings/LLM) não são a mesma coisa que agentes integrados (Claude, Codex, Cursor). A busca
              textual (FTS) funciona sem embeddings.
            </p>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
