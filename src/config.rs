use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{fs, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub port: u16,
    pub no_autostart: bool,
    pub headless: bool,
    pub exit_after: Option<u64>,
}
impl Config {
    pub fn parse() -> Result<Self> {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("无法找到 LOCALAPPDATA")?;
        let mut cfg = Self {
            directory: base.join("GlassTodo"),
            port: 57841,
            no_autostart: false,
            headless: false,
            exit_after: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--data-dir" => {
                    cfg.directory = PathBuf::from(args.next().context("--data-dir 缺少路径")?)
                }
                "--port" => cfg.port = args.next().context("--port 缺少端口")?.parse()?,
                "--no-autostart" => cfg.no_autostart = true,
                "--headless" => cfg.headless = true,
                "--quit" => {} // Handled before normal startup in main.
                "--exit-after" => {
                    cfg.exit_after = Some(args.next().context("--exit-after 缺少秒数")?.parse()?)
                }
                "--help" | "-h" => {
                    println!(
                        "GlassTodo [--data-dir PATH] [--port 57841] [--no-autostart] [--headless] [--exit-after SECONDS] [--quit]"
                    );
                    std::process::exit(0)
                }
                _ => bail!("未知参数 {arg}"),
            }
        }
        ensure_port(cfg.port)?;
        fs::create_dir_all(&cfg.directory)?;
        Ok(cfg)
    }
    pub fn token(&self) -> Result<String> {
        let path = self.directory.join("mcp-token.txt");
        if path.exists() {
            let token = fs::read_to_string(&path)?;
            let token = token.trim();
            if token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Ok(token.to_owned());
            }
            bail!("MCP token 文件格式错误，请检查 {}", path.display())
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("生成本机 MCP 密钥失败: {e}"))?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        fs::write(path, &token)?;
        Ok(token)
    }
    pub fn write_client_examples(&self, token: &str) -> Result<()> {
        let dir = self.directory.join("mcp-configs");
        fs::create_dir_all(&dir)?;
        let url = format!("http://127.0.0.1:{}/mcp", self.port);
        fs::write(
            dir.join("codex.toml"),
            format!(
                "# 合并到用户自己的 Codex config.toml；此文件含本机访问密钥，请勿提交到仓库。\n[mcp_servers.glass_todo]\nurl = \"{url}\"\nhttp_headers = {{ Authorization = \"Bearer {token}\" }}\n"
            ),
        )?;
        for (name, entry) in [
            (
                "cursor.json",
                json!({"url":url,"headers":{"Authorization":format!("Bearer {token}")}}),
            ),
            (
                "claude-code.json",
                json!({"type":"http","url":url,"headers":{"Authorization":format!("Bearer {token}")}}),
            ),
        ] {
            fs::write(
                dir.join(name),
                serde_json::to_vec_pretty(&json!({"mcpServers":{"glass_todo":entry}}))?,
            )?;
        }
        Ok(())
    }
    pub fn log(&self, msg: &str) {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join("app.log"))
        {
            let _ = writeln!(f, "{msg}");
        }
    }
}
fn ensure_port(port: u16) -> Result<()> {
    if port == 0 {
        bail!("MCP 端口不能为 0")
    }
    Ok(())
}
