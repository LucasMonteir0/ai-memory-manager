import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import type * as T from "./types";

/**
 * Modo demonstração (somente `npm run dev:demo`): usa fixtures para screenshots/testes de UI.
 * Em build de produção `import.meta.env.MODE` é "production" e o ramo é eliminado.
 */
const DEMO = import.meta.env.MODE === "demo";

async function invoke<R>(cmd: string, args?: Record<string, unknown>): Promise<R> {
  if (DEMO) {
    const demo = await import("./demo");
    return demo.demoInvoke(cmd, args) as Promise<R>;
  }
  return tauriInvoke<R>(cmd, args);
}

export async function listenOpLines(cb: (l: T.OpLine) => void): Promise<UnlistenFn> {
  if (DEMO) return () => {};
  return tauriListen<T.OpLine>("op-line", (e) => cb(e.payload));
}

export const isDemo = DEMO;

export const api = {
  envInfo: () => invoke<T.EnvInfo>("env_info"),
  detect: () => invoke<T.Detection>("detect"),
  setManualBinary: (path: string | null) => invoke<string>("set_manual_binary", { path }),
  serverHealth: () => invoke<T.Health>("server_health"),

  listProjects: () => invoke<T.Project[]>("list_projects"),
  inspectFolder: (path: string) => invoke<T.FolderInspection>("inspect_folder", { path }),
  addProject: (path: string, displayName?: string) => invoke<T.Project>("add_project", { path, displayName }),
  removeProject: (id: string) => invoke<string>("remove_project", { id }),
  renameProject: (id: string, displayName: string) => invoke<T.Project>("rename_project", { id, displayName }),
  runDoctor: (projectId: string) => invoke<T.DoctorRun>("run_doctor", { projectId }),
  cachedDoctor: (projectId: string) => invoke<[T.Doctor, string] | null>("cached_doctor", { projectId }),
  planMarker: (projectId: string, workspace: string, project: string) =>
    invoke<T.ChangePlan>("plan_marker", { projectId, workspace, project }),

  backfillDryRun: (projectId: string, force: boolean, maxSessions: number) =>
    invoke<T.BackfillRun>("backfill_dry_run", { projectId, force, maxSessions }),
  backfillRun: (projectId: string, force: boolean, maxSessions: number, opId: string) =>
    invoke<T.BackfillRun>("backfill_run", { projectId, force, maxSessions, opId }),
  cancelOperation: (opId: string) => invoke<boolean>("cancel_operation", { opId }),
  recoverRun: (opId: string) => invoke<T.CmdOutput>("recover_run", { opId }),

  agentsView: (projectId: string | null, refresh: boolean) => invoke<T.AgentsView>("agents_view", { projectId, refresh }),
  planAgentChange: (agent: string, component: "mcp" | "hooks", action: "enable" | "disable") =>
    invoke<T.ChangePlan>("plan_agent_change", { agent, component, action }),
  planCodexTokenFix: () => invoke<T.ChangePlan>("plan_codex_token_fix"),
  planCodexBridge: () => invoke<T.ChangePlan>("plan_codex_bridge"),
  planCursorCompatHooks: () => invoke<T.ChangePlan>("plan_cursor_compat_hooks"),
  discardPlan: (planId: string) => invoke<void>("discard_plan", { planId }),
  applyPlan: (planId: string) => invoke<T.ApplyResult>("apply_plan", { planId }),
  testMcp: () => invoke<T.McpTests>("test_mcp"),
  captureTestStart: (agent: string, projectId: string) => invoke<T.CaptureEvidence>("capture_test_start", { agent, projectId }),
  captureTestCheck: (agent: string) => invoke<T.CaptureEvidence>("capture_test_check", { agent }),

  serverStop: () => invoke<string>("server_stop"),
  serverStart: () => invoke<string>("server_start"),
  planAutostart: (enable: boolean) => invoke<T.ChangePlan>("plan_autostart", { enable }),

  listBackups: () => invoke<T.BackupSet[]>("list_backups"),
  planRestoreBackup: (id: string) => invoke<T.ChangePlan>("plan_restore_backup", { id }),
  listOperations: (limit?: number) => invoke<T.AppOperation[]>("list_operations", { limit }),
  operationDetail: (id: string) => invoke<string | null>("operation_detail", { id }),
  sanitizedReport: () => invoke<string>("sanitized_report"),

  aiMemoryRelease: () => invoke<T.ReleaseView>("ai_memory_release"),
  installAiMemory: (addToPath: boolean) => invoke<T.InstallReport>("install_ai_memory", { addToPath }),
  setupPlan: () => invoke<T.SetupPlanView>("setup_plan"),
  setupApply: (agentsSel: string[], opId: string) => invoke<T.CmdOutput>("setup_apply", { agentsSel, opId }),
  upgradeAiMemory: (opId: string) => invoke<T.CmdOutput>("upgrade_ai_memory", { opId }),
  planUninstall: (only: string) => invoke<T.ChangePlan>("plan_uninstall", { only }),
};

export function newOpId(): string {
  return crypto.randomUUID();
}

export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
