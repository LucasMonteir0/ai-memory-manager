// Tipos espelhando os comandos Rust (src-tauri/src/commands.rs). Nenhum contém o token.

export type Interpreted<T> = { kind: "ok"; data: T } | { kind: "raw"; raw: string; reason: string };

export interface CmdOutput {
  command: string;
  cwd?: string | null;
  exitCode?: number | null;
  stdout: string;
  stderr: string;
  timedOut: boolean;
  cancelled: boolean;
  durationMs: number;
}

export interface EnvInfo {
  os: string;
  arch: string;
  nativeWindows: boolean;
  appDir: string;
  appVersion: string;
  installKind: "installed" | "portable";
  claudeConfigDirEnv?: string | null;
  codexHomeEnv?: string | null;
}

export interface BinaryCandidate {
  path: string;
  source: string;
  valid: boolean;
  version?: string | null;
  error?: string | null;
}

export interface Health {
  url: string;
  reachable: boolean;
  httpStatus?: number | null;
  latencyMs?: number | null;
  error?: string | null;
  checkedAt: string;
}

export interface ServiceState {
  runValuePresent: boolean;
  runValue?: string | null;
  runValuePointsToBinary?: boolean | null;
  supervisorPid?: number | null;
  serverPid?: number | null;
  supervisorAlive?: boolean | null;
  serverAlive?: boolean | null;
  stopFilePresent: boolean;
  managedBy: string;
  notes: string[];
}

export interface AiMemoryConfig {
  configPath: string;
  exists: boolean;
  bind?: string | null;
  serverUrl?: string | null;
  bearerTokenFile?: string | null;
  parseError?: string | null;
}

export interface Status {
  version?: string | null;
  dataDir?: string | null;
  bind?: string | null;
  serverUrl?: string | null;
  clientAuth?: boolean | null;
  authPosture?: string | null;
  authFix?: string | null;
  pages?: number | null;
  sessions?: number | null;
  observations?: number | null;
  ftsPagesOk?: boolean | null;
  ftsObservationsOk?: boolean | null;
  embeddingStatus?: string | null;
  embeddingRows?: number | null;
  llmStatus?: string | null;
  spoolPending?: number | null;
  spoolOldestAgeMs?: number | null;
  spoolCapacity?: number | null;
  ingestAccepted?: number | null;
  ingestFailed?: number | null;
  ingestDroppedUnauthorized?: number | null;
  lastPersistedMs?: number | null;
  captureMode?: string | null;
  databaseBytes?: number | null;
  consolidationFailed?: number | null;
  recoveryJournalPending?: number | null;
}

export type DetectionState =
  | "not_installed"
  | "invalid"
  | "unsupported_version"
  | "no_setup"
  | "server_stopped"
  | "auth_error"
  | "healthy"
  | "unknown";

export interface Detection {
  state: DetectionState;
  stateLabel: string;
  binary?: string | null;
  candidates: BinaryCandidate[];
  version?: string | null;
  versionSupport?: "supported" | "unsupported" | "untested" | null;
  commands: string[];
  hasRecover: boolean;
  hasHookDrain: boolean;
  dataDir: string;
  config: AiMemoryConfig;
  serverUrl: string;
  webUrl: string;
  health?: Health | null;
  service?: ServiceState | null;
  status?: Interpreted<Status> | null;
  statusCommand?: CmdOutput | null;
  tokenAvailable: boolean;
  checkedAt: string;
}

export interface Project {
  id: string;
  displayName: string;
  absolutePath: string;
  addedAt: string;
  lastDoctorAt?: string | null;
  lastWorkspace?: string | null;
  lastProject?: string | null;
}

export interface MarkerInfo {
  path: string;
  inSelectedFolder: boolean;
  workspace?: string | null;
  project?: string | null;
  identity?: string | null;
  captureOnly: boolean;
  hasIgnorePaths: boolean;
  parseError?: string | null;
}

export interface FolderInspection {
  input: string;
  resolvedPath: string;
  isDir: boolean;
  isGit: boolean;
  gitRoot?: string | null;
  gitRootDiffers: boolean;
  subRepos: { name: string; path: string }[];
  marker?: MarkerInfo | null;
  suggestedWorkspace: string;
  suggestedProject: string;
  warnings: string[];
}

export interface DoctorRow {
  agent: string;
  localTotal: number;
  localRecent: number;
  captured: number;
  uncaptured: boolean;
}

export interface Doctor {
  workspace: string;
  project: string;
  server?: string | null;
  sinceDays?: number | null;
  rows: DoctorRow[];
  captureCoverageProblem?: string | null;
  markerCaptureProblem?: string | null;
  uncapturedAgents: string[];
  markerStatus?: string | null;
  scopeSource?: string | null;
  serverProjectStatus?: string | null;
  identityLevel?: string | null;
  serverUnreachable: boolean;
  spoolPending?: number | null;
  spoolOldestAgeMs?: number | null;
  codexHooksTrusted: string[];
  codexHooksUntrusted: string[];
  plugins: string[];
  nativeMemory: { agent: string; fileCount: number; status: string }[];
  authPosture?: string | null;
  warnings: string[];
}

export interface DoctorRun {
  projectId: string;
  cwd: string;
  ranAt: string;
  testKind: string;
  output: CmdOutput;
  parsed: Interpreted<Doctor>;
}

export interface Backfill {
  workspace?: string | null;
  project?: string | null;
  selected: number;
  importedSessions: number;
  importedEvents: number;
  skippedForCap: number;
  failedSessions: number;
  skippedNonEmpty: boolean;
  skippedNotAdmitted: boolean;
  dryRun: boolean;
}

export interface BackfillRun {
  output: CmdOutput;
  parsed: Interpreted<Backfill>;
  dryRun: boolean;
  ranAt: string;
}

export type IntegrationStatus =
  | "not_detected"
  | "not_configured"
  | "configured"
  | "verified"
  | "untrusted"
  | "partial"
  | "error"
  | "unsupported";

export interface Component {
  status: IntegrationStatus;
  detail: string;
  configPath?: string | null;
  source?: string | null;
  evidence: string[];
  canConfigure: boolean;
  canDisable: boolean;
  blockedReason?: string | null;
  experimental?: string | null;
}

export interface AppInfo {
  installed?: boolean | null;
  path?: string | null;
  version?: string | null;
  source?: string | null;
}

export interface AgentReport {
  id: string;
  displayName: string;
  description: string;
  mcpClientId?: string | null;
  hooksAgentId?: string | null;
  app: AppInfo;
  mcp: Component;
  hooks: Component;
  hooksScope: string;
  notes: string[];
  trustedEvents: string[];
  untrustedEvents: string[];
  restartHint: string;
  docsUrl: string;
}

export interface CaptureEvidence {
  agent: string;
  projectId: string;
  startedAt: string;
  capturedBefore: number;
  capturedAfter?: number | null;
  checkedAt?: string | null;
  verified: boolean;
  message: string;
}

export interface DesktopApps {
  claude: AppInfo;
  codex: AppInfo;
  cursor: AppInfo;
  nodeAvailable: boolean;
  gitBash?: string | null;
}

export interface AgentsView {
  apps: DesktopApps;
  agents: AgentReport[];
  doctorSource?: string | null;
  captureTests: CaptureEvidence[];
}

export interface FileDiff {
  path: string;
  existedBefore: boolean;
  changed: boolean;
  unified: string;
  added: number;
  removed: number;
}

export interface ChangePlan {
  id: string;
  title: string;
  explanation: string;
  agentId?: string | null;
  projectId?: string | null;
  commands: string[];
  files: string[];
  diffs: FileDiff[];
  dryRunOutput?: string | null;
  warnings: string[];
  afterSteps: string[];
  destructive: boolean;
}

export interface ApplyResult {
  success: boolean;
  summary: string;
  backupId?: string | null;
  rolledBack: boolean;
  diffs: FileDiff[];
  output?: CmdOutput | null;
  verification: string;
  afterSteps: string[];
}

export interface McpTest {
  level: string;
  endpoint: string;
  withToken: boolean;
  ok: boolean;
  httpStatus?: number | null;
  serverName?: string | null;
  serverVersion?: string | null;
  toolCount?: number | null;
  toolNames: string[];
  error?: string | null;
  testedAt: string;
}

export interface McpTests {
  withToken: McpTest;
  withoutToken: McpTest;
  interpretation: string;
}

export interface BackupSet {
  id: string;
  createdAt: string;
  reason: string;
  agentId?: string | null;
  files: { originalPath: string; backupName?: string | null; existed: boolean; sha256?: string | null }[];
  registry: { key: string; name: string; value?: string | null; secret: boolean }[];
}

export interface AppOperation {
  id: string;
  action: string;
  kind: "leitura" | "alteracao" | "teste";
  projectId?: string | null;
  agentId?: string | null;
  startedAt: string;
  completedAt?: string | null;
  success?: boolean | null;
  summary?: string | null;
  backupId?: string | null;
}

export interface ReleaseInfo {
  repo: string;
  tag: string;
  version?: string | null;
  publishedAt?: string | null;
  htmlUrl: string;
  changelog: string;
  assetName?: string | null;
  assetUrl?: string | null;
  assetSize?: number | null;
  sha256Url?: string | null;
  publisher: string;
}

export interface ReleaseView {
  release: ReleaseInfo;
  installedVersion?: string | null;
  updateAvailable: boolean;
  targetDir: string;
}

export interface InstallReport {
  installedTo: string;
  version: string;
  sha256: string;
  addedToPath: boolean;
  steps: string[];
}

export interface SetupPlan {
  fresh: boolean;
  dataDir?: string | null;
  configPath?: string | null;
  serviceAction?: string | null;
  serviceManager?: string | null;
  servicePath?: string | null;
  agents: { agent: string; foundKind?: string | null; foundPath?: string | null; wiring?: string | null }[];
  otherAgents: string[];
}

export interface SetupPlanView {
  parsed: Interpreted<SetupPlan>;
  output: CmdOutput;
  alreadyConfigured: boolean;
}

export interface OpLine {
  opId: string;
  stream: "stdout" | "stderr";
  line: string;
}
