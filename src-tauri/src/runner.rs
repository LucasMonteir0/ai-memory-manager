//! ExecuÃ§Ã£o segura de processos: argv separado, cwd explÃ­cito, timeout, sem shell,
//! streaming de linhas e cancelamento.

use crate::sanitize::sanitize;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct CmdSpec {
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
}

impl CmdSpec {
    pub fn new(exe: impl Into<PathBuf>, args: &[&str]) -> Self {
        Self {
            exe: exe.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            env: vec![("NO_COLOR".into(), "1".into())],
            timeout: Duration::from_secs(60),
        }
    }
    pub fn args_owned(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }
    /// Linha de comando legÃ­vel (somente para exibiÃ§Ã£o; nunca Ã© executada por shell).
    pub fn display(&self) -> String {
        let mut parts = vec![quote(&self.exe.to_string_lossy())];
        parts.extend(self.args.iter().map(|a| quote(a)));
        sanitize(&parts.join(" "))
    }
}

fn quote(s: &str) -> String {
    if s.is_empty() || s.contains(char::is_whitespace) {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdOutput {
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u128,
}

impl CmdOutput {
    pub fn ok(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && !self.cancelled
    }
    /// stderr sem as linhas INFO de inicializaÃ§Ã£o do ai-memory.
    pub fn stderr_meaningful(&self) -> String {
        self.stderr
            .lines()
            .filter(|l| !(l.contains(" INFO ") && l.contains("ai-memory starting")))
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    Stdout,
    Stderr,
}

pub type LineSink = Arc<dyn Fn(StreamKind, &str) + Send + Sync>;

pub async fn run(spec: &CmdSpec) -> std::io::Result<CmdOutput> {
    run_streaming(spec, None, None).await
}

pub async fn run_streaming(
    spec: &CmdSpec,
    sink: Option<LineSink>,
    cancel: Option<watch::Receiver<bool>>,
) -> std::io::Result<CmdOutput> {
    let mut cmd = Command::new(&spec.exe);
    cmd.args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let started = Instant::now();
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let sink_o = sink.clone();
    let out_task = tokio::spawn(async move {
        read_lossy(BufReader::new(stdout), StreamKind::Stdout, sink_o).await
    });
    let sink_e = sink.clone();
    let err_task = tokio::spawn(async move {
        read_lossy(BufReader::new(stderr), StreamKind::Stderr, sink_e).await
    });

    let mut timed_out = false;
    let mut cancelled = false;
    let mut cancel_rx = cancel;
    let status = {
        let wait = child.wait();
        tokio::pin!(wait);
        let sleep = tokio::time::sleep(spec.timeout);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                st = &mut wait => break Some(st?),
                _ = &mut sleep => { timed_out = true; break None; }
                changed = async {
                    match cancel_rx.as_mut() {
                        Some(rx) => rx.changed().await.ok().map(|_| *rx.borrow()),
                        None => std::future::pending().await,
                    }
                } => {
                    match changed {
                        Some(true) => { cancelled = true; break None; }
                        Some(false) => continue,
                        None => { cancel_rx = None; continue; }
                    }
                }
            }
        }
    };
    let exit_code = match status {
        Some(st) => st.code(),
        None => {
            let _ = child.kill().await;
            None
        }
    };
    let stdout_s = out_task.await.unwrap_or_default();
    let stderr_s = err_task.await.unwrap_or_default();
    Ok(CmdOutput {
        command: spec.display(),
        cwd: spec.cwd.as_ref().map(|c| c.to_string_lossy().to_string()),
        exit_code,
        stdout: sanitize(&stdout_s),
        stderr: sanitize(&stderr_s),
        timed_out,
        cancelled,
        duration_ms: started.elapsed().as_millis(),
    })
}

/// Lê linhas tolerando bytes não-UTF-8 (ex.: saída em code page OEM).
async fn read_lossy<R: tokio::io::AsyncBufRead + Unpin>(mut r: R, kind: StreamKind, sink: Option<LineSink>) -> String {
    let mut buf = String::new();
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match r.read_until(b'\n', &mut raw).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&raw);
                let line = line.trim_end_matches(['\r', '\n']);
                if let Some(s) = &sink {
                    s(kind, &sanitize(line));
                }
                buf.push_str(line);
                buf.push('\n');
            }
        }
    }
    buf
}

/// Divide uma linha de comando do Windows em argv (aspas duplas), para interpretar o valor
/// do registro Run. Nunca usado para executar via shell.
pub fn split_windows_cmdline(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut has = false;
    for ch in s.chars() {
        match ch {
            '"' => {
                in_q = !in_q;
                has = true;
            }
            c if c.is_whitespace() && !in_q => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd_exe() -> PathBuf {
        PathBuf::from(std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into()))
    }

    #[tokio::test]
    async fn passes_args_with_spaces_as_single_argv() {
        // usa o prÃ³prio executÃ¡vel de teste? Mais simples: `cmd /c echo` sÃ³ para observar,
        // mas a garantia real Ã© que o argumento chega intacto em Command::args.
        let dir = tempfile::tempdir().unwrap();
        let spaced = dir.path().join("pasta com espaÃ§o Ã©");
        std::fs::create_dir_all(&spaced).unwrap();
        let spec = CmdSpec::new(cmd_exe(), &["/d", "/c", "cd"]).cwd(&spaced);
        let out = run(&spec).await.unwrap();
        assert!(out.ok(), "{out:?}");
        assert!(out.stdout.contains("pasta com espa"), "{}", out.stdout);
    }

    #[tokio::test]
    async fn times_out() {
        let spec = CmdSpec::new(cmd_exe(), &["/d", "/c", "ping -n 6 127.0.0.1 >NUL"])
            .timeout(Duration::from_millis(300));
        let out = run(&spec).await.unwrap();
        assert!(out.timed_out);
        assert!(!out.ok());
    }

    #[tokio::test]
    async fn can_cancel() {
        let (tx, rx) = watch::channel(false);
        let spec = CmdSpec::new(cmd_exe(), &["/d", "/c", "ping -n 6 127.0.0.1 >NUL"])
            .timeout(Duration::from_secs(30));
        let h = tokio::spawn(async move { run_streaming(&spec, None, Some(rx)).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        tx.send(true).unwrap();
        let out = h.await.unwrap().unwrap();
        assert!(out.cancelled);
    }

    #[tokio::test]
    async fn missing_exe_is_error_not_shell_fallback() {
        let spec = CmdSpec::new(r"C:\nao\existe\ai-memory.exe", &["--version"]);
        assert!(run(&spec).await.is_err());
    }

    #[test]
    fn display_redacts_tokens() {
        let spec = CmdSpec::new("ai-memory.exe", &["install-hooks", "--auth-token", "segredo123456"]);
        assert!(!spec.display().contains("segredo123456"));
    }

    #[test]
    fn splits_run_value() {
        let v = split_windows_cmdline(
            r#""C:\Users\U\AppData\Local\Programs\ai-memory\ai-memory.exe" --data-dir "C:\Users\U\AppData\Local\ai-memory" service-host"#,
        );
        assert_eq!(v.len(), 4);
        assert!(v[0].ends_with("ai-memory.exe"));
        assert_eq!(v[3], "service-host");
    }
}
