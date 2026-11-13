//! Redação de segredos antes de qualquer texto chegar à UI, a logs ou a exportações.

use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::RwLock;

/// Segredos conhecidos em tempo de execução (ex.: token bearer lido do ai-memory).
/// Ficam só na memória do processo; nunca são persistidos.
static KNOWN_SECRETS: Lazy<RwLock<Vec<String>>> = Lazy::new(|| RwLock::new(Vec::new()));

pub const REDACTED: &str = "[REDIGIDO]";

pub fn register_secret(secret: &str) {
    let s = secret.trim();
    if s.len() < 6 {
        return;
    }
    if let Ok(mut list) = KNOWN_SECRETS.write() {
        if !list.iter().any(|x| x == s) {
            list.push(s.to_string());
        }
    }
}

static PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        // Authorization: Bearer xxx / authorization = "..."
        (
            Regex::new(r#"(?i)(authorization["']?\s*[:=]\s*["']?)(bearer\s+)?[^\s"',}]+"#).unwrap(),
            "${1}${2}[REDIGIDO]",
        ),
        (
            Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=\-]{8,}").unwrap(),
            "Bearer [REDIGIDO]",
        ),
        // --auth-token <valor>
        (
            Regex::new(r#"(?i)(--auth-token[=\s]+)["']?[^\s"']+"#).unwrap(),
            "${1}[REDIGIDO]",
        ),
        // chave = valor para nomes sensíveis (exceto *_env_var/_file, que são nomes/caminhos)
        (
            Regex::new(
                r#"(?i)\b((?:[a-z0-9_\-]*?)(?:token|api[_\-]?key|secret|password|passwd|pepper|cookie|session[_\-]?key))(["']?\s*[:=]\s*["']?)([^\s"',}\]]{4,})"#,
            )
            .unwrap(),
            "${1}${2}[REDIGIDO]",
        ),
        // parâmetros de URL sensíveis
        (
            Regex::new(r"(?i)([?&](?:token|key|apikey|api_key|access_token|auth)=)[^&\s]+").unwrap(),
            "${1}[REDIGIDO]",
        ),
        // chaves com prefixos conhecidos
        (
            Regex::new(r"\b(sk-[A-Za-z0-9_\-]{12,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abpr]-[A-Za-z0-9\-]{10,})").unwrap(),
            "[REDIGIDO]",
        ),
    ]
});

/// Nomes de chave que contêm "token" mas não são segredos (referências/caminhos/flags).
static SAFE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(env_var|_file|_path|file|tokens?_total|token_usage|max_tokens|tokenizer|bearer_token_env_var)$").unwrap()
});

pub fn sanitize(input: &str) -> String {
    let mut out = input.to_string();
    if let Ok(list) = KNOWN_SECRETS.read() {
        for s in list.iter() {
            if !s.is_empty() {
                out = out.replace(s.as_str(), REDACTED);
            }
        }
    }
    for (i, (re, rep)) in PATTERNS.iter().enumerate() {
        if i == 3 {
            // regra de chave=valor: preserva chaves seguras
            out = re
                .replace_all(&out, |caps: &regex::Captures| {
                    let key = &caps[1];
                    if SAFE_KEY.is_match(key) {
                        caps[0].to_string()
                    } else {
                        format!("{}{}{}", &caps[1], &caps[2], REDACTED)
                    }
                })
                .into_owned();
        } else {
            out = re.replace_all(&out, *rep).into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_header() {
        let s = sanitize("Authorization: Bearer abcdef1234567890XYZ");
        assert!(!s.contains("abcdef1234567890XYZ"), "{s}");
    }

    #[test]
    fn redacts_auth_token_flag() {
        let s = sanitize("ai-memory install-hooks --auth-token supersecretvalue --apply");
        assert!(!s.contains("supersecretvalue"), "{s}");
        assert!(s.contains("--apply"));
    }

    #[test]
    fn redacts_key_value_pairs_but_keeps_env_var_names() {
        let s = sanitize("token_pepper = \"abc123secretpepper\"\nbearer_token_env_var = \"AI_MEMORY_AUTH_TOKEN\"");
        assert!(!s.contains("abc123secretpepper"), "{s}");
        assert!(s.contains("AI_MEMORY_AUTH_TOKEN"), "{s}");
    }

    #[test]
    fn redacts_registered_secret_anywhere() {
        register_secret("zz-registered-secret-0001");
        let s = sanitize("algo zz-registered-secret-0001 fim");
        assert_eq!(s, "algo [REDIGIDO] fim");
    }

    #[test]
    fn keeps_normal_paths_and_counts() {
        let s = sanitize(r#"{"sessions": 28, "path": "F:\\repos\\projeto-a", "auth": true}"#);
        assert!(s.contains("28") && s.contains("projeto-a"));
    }

    #[test]
    fn redacts_url_params() {
        let s = sanitize("http://127.0.0.1:49374/web?token=abcd1234efgh");
        assert!(!s.contains("abcd1234efgh"), "{s}");
    }
}
