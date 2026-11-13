import * as React from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import { Activity, Bot, BrainCircuit, FolderGit2, LayoutDashboard, Settings as SettingsIcon } from "lucide-react";
import { PlannerProvider } from "@/components/Planner";
import { Overview } from "@/pages/Overview";
import { Projects } from "@/pages/Projects";
import { Agents } from "@/pages/Agents";
import { Memories } from "@/pages/Memories";
import { Activities } from "@/pages/Activities";
import { Settings } from "@/pages/Settings";
import { isDemo } from "@/lib/api";
import { cn } from "@/lib/utils";

export type Page = "overview" | "projects" | "agents" | "memories" | "activities" | "settings";

const NAV: { id: Page; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: "overview", label: "Visão geral", icon: LayoutDashboard },
  { id: "projects", label: "Projetos", icon: FolderGit2 },
  { id: "agents", label: "Agentes", icon: Bot },
  { id: "memories", label: "Memórias", icon: BrainCircuit },
  { id: "activities", label: "Atividades", icon: Activity },
  { id: "settings", label: "Configurações", icon: SettingsIcon },
];

const qc = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: false, retry: 0, staleTime: 15_000 } },
});

function initialTheme(): "light" | "dark" {
  try {
    const t = localStorage.getItem("aimm-theme");
    if (t === "light" || t === "dark") return t;
  } catch {
    /* sem armazenamento */
  }
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export default function App() {
  const [page, setPage] = React.useState<Page>(() => {
    const h = window.location.hash.replace("#", "") as Page;
    return NAV.some((n) => n.id === h) ? h : "overview";
  });
  const [theme, setTheme] = React.useState<"light" | "dark">(initialTheme);
  React.useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    try {
      localStorage.setItem("aimm-theme", theme);
    } catch {
      /* ignore */
    }
  }, [theme]);
  React.useEffect(() => {
    if (window.location.hash !== `#${page}`) window.location.hash = page;
  }, [page]);
  React.useEffect(() => {
    const on = () => {
      const h = window.location.hash.replace("#", "") as Page;
      if (NAV.some((n) => n.id === h)) setPage(h);
    };
    window.addEventListener("hashchange", on);
    return () => window.removeEventListener("hashchange", on);
  }, []);

  return (
    <QueryClientProvider client={qc}>
      <PlannerProvider>
        <div className="flex h-full">
          <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-panel">
            <div className="flex items-center gap-2 px-4 py-4">
              <BrainCircuit className="size-5 text-accent" />
              <div className="leading-tight">
                <div className="text-[14px] font-semibold">AI Memory Manager</div>
                <div className="text-[11px] text-muted">gerenciador do ai-memory</div>
              </div>
            </div>
            <nav className="flex-1 space-y-0.5 px-2" aria-label="Navegação principal">
              {NAV.map((n) => (
                <button
                  key={n.id}
                  onClick={() => setPage(n.id)}
                  aria-current={page === n.id ? "page" : undefined}
                  className={cn(
                    "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-left text-[13px]",
                    page === n.id ? "bg-panel-2 font-semibold text-fg" : "text-muted hover:bg-panel-2 hover:text-fg",
                  )}
                >
                  <n.icon className="size-4" />
                  {n.label}
                </button>
              ))}
            </nav>
            {isDemo && <div className="m-3 rounded-md bg-warn/15 px-2 py-1 text-center text-[11px] text-warn">MODO DEMONSTRAÇÃO (fixtures)</div>}
          </aside>
          <main className="scroll-thin flex-1 overflow-y-auto">
            <div className="mx-auto max-w-6xl px-6 py-6">
              {page === "overview" && <Overview go={setPage} />}
              {page === "projects" && <Projects />}
              {page === "agents" && <Agents go={setPage} />}
              {page === "memories" && <Memories />}
              {page === "activities" && <Activities />}
              {page === "settings" && <Settings theme={theme} setTheme={setTheme} />}
            </div>
          </main>
        </div>
        <Toaster richColors position="bottom-right" theme={theme} />
      </PlannerProvider>
    </QueryClientProvider>
  );
}
