//! Backend configuration from the environment (set by Terraform on Cloud Run).

#[derive(Clone, Debug)]
pub struct Config {
    pub project_id: String,
    pub region: String,
    pub shop_url: String,
    pub monitoring_url: String,
    pub github_owner: String,
    pub github_repo: String,
    pub github_token: String,
    pub github_webhook_secret: String,
    pub auto_approve: bool,
    pub ui_dist: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            project_id: env("PROJECT_ID", ""),
            region: env("REGION", "us-central1"),
            shop_url: env("SHOP_URL", ""),
            monitoring_url: env("MONITORING_URL", ""),
            github_owner: env("GITHUB_OWNER", ""),
            github_repo: env("GITHUB_REPO", ""),
            github_token: env("GITHUB_TOKEN", ""),
            github_webhook_secret: env("GITHUB_WEBHOOK_SECRET", ""),
            auto_approve: env("AUTO_APPROVE", "false") == "true",
            ui_dist: env("UI_DIST", "dist"),
            port: env("PORT", "8080").parse().unwrap_or(8080),
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}
