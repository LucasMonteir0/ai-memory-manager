// Fixtures SOMENTE para `npm run dev:demo` (screenshots/testes de UI). Não entra no build de produção.
// Cenário via query string: ?cenario=ausente | parado | (padrão: instalado)
import type * as T from "./types";

const scenario = new URLSearchParams(window.location.search).get("cenario") ?? "instalado";
const now = new Date().toISOString();

const status: T.Status = {
  version: "3.4.6",
  dataDir: "C:\\Users\\usuario\\AppData\\Local\\ai-memory",
  serverUrl: "http://127.0.0.1:49374",
  clientAuth: true,
  authPosture: "token",
  pages: 13,
  sessions: 25,
  observations: 864,
  ftsPagesOk: true,
  ftsObservationsOk: true,
  embeddingStatus: "disabled",
  embeddingRows: 0,
  llmStatus: "disabled",
  spoolPending: 0,
  spoolOldestAgeMs: null,
  spoolCapacity: 10000,
  ingestAccepted: 865,
  ingestFailed: 0,
  ingestDroppedUnauthorized: 0,
  lastPersistedMs: Date.now() - 90_000,
  captureMode: "denylist",
  databaseBytes: 3_194_880,
};

function detection(): T.Detection {
  const base: T.Detection = {
    state: "healthy",
    stateLabel: "Instalado e saudável",
    binary: "C:\\Users\\usuario\\AppData\\Local\\Programs\\ai-memory\\ai-memory.exe",
    candidates: [{ path: "C:\\Users\\usuario\\AppData\\Local\\Programs\\ai-memory\\ai-memory.exe", source: "padrao", valid: true, version: "3.4.6" }],
    version: "3.4.6",
    versionSupport: "supported",
    commands: ["setup", "status", "doctor", "backfill", "recover", "install-mcp", "install-hooks", "uninstall", "upgrade", "serve"],
    hasRecover: true,
    hasHookDrain: false,
    dataDir: "C:\\Users\\usuario\\AppData\\Local\\ai-memory",
    config: { configPath: "C:\\Users\\usuario\\AppData\\Local\\ai-memory\\config.toml", exists: true, bind: "127.0.0.1:49374" },
    serverUrl: "http://127.0.0.1:49374",
    webUrl: "http://127.0.0.1:49374/web",
    health: { url: "http://127.0.0.1:49374/healthz", reachable: true, httpStatus: 200, latencyMs: 3, checkedAt: now },
    service: { runValuePresent: true, runValuePointsToBinary: true, supervisorPid: 17624, serverPid: 2620, supervisorAlive: true, serverAlive: true, stopFilePresent: false, managedBy: "service-host", notes: [] },
    status: { kind: "ok", data: status },
    tokenAvailable: true,
    checkedAt: now,
  };
  if (scenario === "ausente")
    return { ...base, state: "not_installed", stateLabel: "Não instalado", binary: null, version: null, versionSupport: null, candidates: [], health: null, service: null, status: null, tokenAvailable: false, config: { configPath: "", exists: false } };
  if (scenario === "parado")
    return { ...base, state: "server_stopped", stateLabel: "Instalado com servidor parado", health: { url: base.health!.url, reachable: false, error: "connection refused", checkedAt: now }, status: { kind: "raw", raw: "", reason: "servidor inacessível" } };
  return base;
}

const projects: T.Project[] = [
  { id: "p1", displayName: "projeto-a", absolutePath: "F:\\repos\\projeto-a", addedAt: now, lastDoctorAt: now, lastWorkspace: "default", lastProject: "projeto-a" },
  { id: "p2", displayName: "projeto-b", absolutePath: "F:\\repos\\projeto-b", addedAt: now, lastDoctorAt: null },
];

const doctorA: T.Doctor = {
  workspace: "default",
  project: "projeto-a",
  server: "http://127.0.0.1:49374",
  sinceDays: 30,
  rows: [
    { agent: "claude-code", localTotal: 30, localRecent: 30, captured: 21, uncaptured: false },
    { agent: "codex", localTotal: 3, localRecent: 3, captured: 3, uncaptured: false },
  ],
  uncapturedAgents: [],
  markerStatus: "absent",
  scopeSource: "fallback",
  serverProjectStatus: "exact",
  identityLevel: "root",
  serverUnreachable: false,
  spoolPending: 0,
  codexHooksTrusted: ["sessionStart", "userPromptSubmit", "preToolUse", "postToolUse", "preCompact", "stop", "sessionEnd"],
  codexHooksUntrusted: ["stop"],
  plugins: [],
  nativeMemory: [{ agent: "claude-code", fileCount: 3, status: "not excluded" }],
  authPosture: "token",
  warnings: ["Codex: hooks não autorizados (stop). Aprove-os no Codex Desktop."],
};

const comp = (p: Partial<T.Component>): T.Component => ({
  status: "configured",
  detail: "",
  evidence: [],
  canConfigure: true,
  canDisable: true,
  ...p,
});

const agentsView: T.AgentsView = {
  apps: {
    claude: { installed: true, version: "5.11492.2.0", source: "MSIX Claude" },
    codex: { installed: true, version: "26.831.2377.0", source: "MSIX OpenAI.Codex" },
    cursor: { installed: true, version: "2.4.1", source: "instalação por usuário" },
    nodeAvailable: true,
    gitBash: "C:\\Program Files\\Git\\bin\\bash.exe",
  },
  doctorSource: `doctor default/projeto-a em ${now}`,
  captureTests: [],
  agents: [
    {
      id: "claude-code",
      displayName: "Claude Desktop — Code",
      description: "Aba Code do Claude Desktop (engine Claude Code).",
      mcpClientId: "claude-code",
      hooksAgentId: "claude-code",
      app: { installed: true, version: "5.11492.2.0", source: "MSIX Claude" },
      mcp: comp({ detail: "Entrada 'ai-memory' via ponte stdio nativa (ai-memory.exe mcp-bridge).", configPath: "C:\\Users\\usuario\\.claude.json", source: "~/.claude.json (MCP manual)", evidence: ["Configuração encontrada (não prova que o app carregou)."] }),
      hooks: comp({ status: "verified", detail: "10 evento(s) de Claude Code registrados.", configPath: "C:\\Users\\usuario\\.claude\\settings.json", source: "arquivo de hooks", evidence: ["Captura confirmada com sessão nova.", "doctor default/projeto-a: 30 sessão(ões) locais, 21 capturada(s)."] }),
      hooksScope: "Global (~/.claude/settings.json).",
      notes: ["A aba Code do Claude Desktop usa a engine do Claude Code."],
      trustedEvents: [],
      untrustedEvents: [],
      cursorHookConflict: 0,
      restartHint: "Reabra o Claude Desktop.",
      docsUrl: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md",
    },
    {
      id: "codex",
      displayName: "Codex Desktop",
      description: "OpenAI Codex (aplicativo desktop).",
      mcpClientId: "codex",
      hooksAgentId: "codex",
      app: { installed: true, version: "26.831.2377.0", source: "MSIX OpenAI.Codex" },
      mcp: comp({ status: "error", detail: "O token é lido da variável AI_MEMORY_AUTH_TOKEN, que não existe no ambiente do usuário. O Codex provavelmente recebe 401 ao chamar o MCP.", configPath: "C:\\Users\\usuario\\.codex\\config.toml", source: "config.toml" }),
      hooks: comp({ status: "untrusted", detail: "Hooks não autorizados: stop. Ação manual: abra o Codex Desktop e aprove os hooks quando solicitado.", configPath: "C:\\Users\\usuario\\.codex\\hooks.json", source: "arquivo de hooks" }),
      hooksScope: "Global (CODEX_HOME).",
      notes: ["Não depende do comando `codex`."],
      trustedEvents: doctorA.codexHooksTrusted,
      untrustedEvents: ["stop"],
      cursorHookConflict: 0,
      restartHint: "Reabra o Codex Desktop.",
      docsUrl: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md",
    },
    {
      id: "cursor",
      displayName: "Cursor",
      description: "Cursor Desktop (agente).",
      mcpClientId: "cursor",
      hooksAgentId: "cursor",
      app: { installed: true, version: "2.4.1", source: "instalação por usuário" },
      mcp: comp({ status: "not_configured", detail: "Arquivo não existe; MCP ai-memory ausente.", configPath: "C:\\Users\\usuario\\.cursor\\mcp.json", canDisable: false }),
      hooks: comp({ status: "not_configured", detail: "Nenhum hook ai-memory registrado.", configPath: "C:\\Users\\usuario\\.cursor\\hooks.json", canDisable: false, experimental: "Experimental no Windows nativo: hooks .sh via Git Bash." }),
      hooksScope: "Global (~/.cursor).",
      notes: [],
      trustedEvents: [],
      untrustedEvents: [],
      cursorHookConflict: 0,
      restartHint: "Reinicie o Cursor.",
      docsUrl: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md",
    },
    {
      id: "claude-desktop",
      displayName: "Claude Desktop — Chat",
      description: "Aba Chat do Claude Desktop (somente MCP).",
      mcpClientId: "claude-desktop",
      hooksAgentId: null,
      app: { installed: true, source: "MSIX Claude" },
      mcp: comp({ status: "not_configured", detail: "Arquivo não existe; MCP ai-memory ausente.", canDisable: false }),
      hooks: comp({ status: "unsupported", detail: "Somente MCP — a aba Chat não oferece hooks de captura.", canConfigure: false, canDisable: false }),
      hooksScope: "—",
      notes: ["Somente MCP: a aba Chat não tem hooks; não há captura automática de conversas."],
      trustedEvents: [],
      untrustedEvents: [],
      cursorHookConflict: 0,
      restartHint: "",
      docsUrl: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md",
    },
  ],
};

const plan: T.ChangePlan = {
  id: "demo-plan",
  title: "Configurar MCP — cursor",
  explanation: "Registra o servidor MCP 'ai-memory' no arquivo do agente usando o instalador oficial (install-mcp --apply).",
  agentId: "cursor",
  commands: ['ai-memory.exe install-mcp --client cursor --config-file C:\\Users\\usuario\\.cursor\\mcp.json --language en --apply'],
  files: ["C:\\Users\\usuario\\.cursor\\mcp.json"],
  diffs: [
    {
      path: "C:\\Users\\usuario\\.cursor\\mcp.json",
      existedBefore: false,
      changed: true,
      added: 9,
      removed: 0,
      unified: '--- antes\n+++ depois\n@@ -0,0 +1,9 @@\n+{\n+  "mcpServers": {\n+    "ai-memory": {\n+      "command": "C:\\\\Users\\\\usuario\\\\AppData\\\\Local\\\\Programs\\\\ai-memory\\\\ai-memory.exe",\n+      "args": ["mcp-bridge", "--no-session-id", "--server-url", "http://127.0.0.1:49374/mcp"]\n+    }\n+  }\n+}\n',
    },
  ],
  dryRunOutput: "# Cursor — ~/.cursor/mcp.json\n{ ... }",
  warnings: ["O dry-run mostra a proposta; ele não comprova que a integração está ativa."],
  afterSteps: ["Reinicie o Cursor.", "Use 'Verificar captura' com uma sessão nova."],
  destructive: false,
};

const ops: T.AppOperation[] = [
  { id: "o1", action: "doctor", kind: "leitura", projectId: "p1", startedAt: now, completedAt: now, success: true, summary: "doctor em F:\\repos\\projeto-a — interpretado" },
  { id: "o2", action: "teste MCP (servidor)", kind: "teste", startedAt: now, completedAt: now, success: true, summary: "Servidor MCP responde e exige token." },
  { id: "o3", action: "Desconectar MCP — cursor", kind: "alteracao", agentId: "cursor", startedAt: now, completedAt: now, success: true, summary: "concluído e verificado", backupId: "20261113-180000-abcd1234" },
];

export async function demoInvoke(cmd: string, _args?: Record<string, unknown>): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 120));
  switch (cmd) {
    case "env_info":
      return { os: "windows", arch: "x86_64", nativeWindows: true, appDir: "C:\\Users\\usuario\\AppData\\Roaming\\AI Memory Manager", appVersion: "0.1.0", installKind: "installed" } satisfies T.EnvInfo;
    case "detect":
      return detection();
    case "list_projects":
      return scenario === "ausente" ? [] : projects;
    case "cached_doctor":
      return _args?.projectId === "p1" ? [doctorA, now] : null;
    case "inspect_folder":
      return { input: "", resolvedPath: "F:\\repos\\projeto-a", isDir: true, isGit: false, gitRootDiffers: false, subRepos: [{ name: "api", path: "" }, { name: "web", path: "" }], marker: null, suggestedWorkspace: "default", suggestedProject: "projeto-a", warnings: ["A pasta não é um repositório git, mas contém 2 sub-repositório(s)."] } satisfies T.FolderInspection;
    case "agents_view":
      return agentsView;
    case "list_operations":
      return ops;
    case "operation_detail":
      return "$ ai-memory.exe doctor --json --language en\n(cwd F:\\repos\\projeto-a)\n{ ... }";
    case "list_backups":
      return [{ id: "20261113-180000-abcd1234", createdAt: now, reason: "Desconectar MCP — cursor", agentId: "cursor", files: [{ originalPath: "C:\\Users\\usuario\\.cursor\\mcp.json", existed: true }], registry: [] }];
    case "plan_agent_change":
    case "plan_marker":
    case "plan_autostart":
      return plan;
    case "test_mcp":
      return {
        withToken: { level: "servidor", endpoint: "http://127.0.0.1:49374/mcp", withToken: true, ok: true, httpStatus: 200, serverName: "ai-memory", serverVersion: "3.4.6", toolCount: 33, toolNames: ["memory_query", "memory_status"], testedAt: now },
        withoutToken: { level: "servidor", endpoint: "http://127.0.0.1:49374/mcp", withToken: false, ok: false, httpStatus: 401, toolNames: [], testedAt: now },
        interpretation: "Servidor MCP responde e exige token: clientes sem token (ex.: variável ausente no Codex) falharão.",
      } satisfies T.McpTests;
    case "backfill_dry_run":
      return { dryRun: true, ranAt: now, output: { command: "ai-memory backfill --dry-run", stdout: "", stderr: "", exitCode: 0, timedOut: false, cancelled: false, durationMs: 900 }, parsed: { kind: "ok", data: { workspace: "default", project: "projeto-a", selected: 25, importedSessions: 0, importedEvents: 0, skippedForCap: 4, failedSessions: 0, skippedNonEmpty: false, skippedNotAdmitted: false, dryRun: true } } } satisfies T.BackfillRun;
    case "ai_memory_release":
      return { installedVersion: scenario === "ausente" ? null : "3.4.6", updateAvailable: false, targetDir: "C:\\Users\\usuario\\AppData\\Local\\Programs\\ai-memory", release: { repo: "akitaonrails/ai-memory", tag: "v3.4.6", version: "3.4.6", publishedAt: now, htmlUrl: "", changelog: "Notas da versão…", assetName: "ai-memory-windows-x86_64.zip", assetSize: 18_400_000, sha256Url: "https://github.com/akitaonrails/ai-memory/releases/download/v3.4.6/ai-memory-windows-x86_64.zip.sha256", publisher: "akitaonrails" } } satisfies T.ReleaseView;
    default:
      throw new Error(`Modo demonstração: '${cmd}' não executa ações reais.`);
  }
}
