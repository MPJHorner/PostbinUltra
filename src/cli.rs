use std::net::IpAddr;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "postbin-ultra",
    version,
    about = "A blazing-fast local request inspector for developers.",
    long_about = "PostbinUltra captures every HTTP request that hits its capture port — any method, any path — and shows them live in your terminal and in a beautiful real-time web UI."
)]
pub struct Cli {
    /// Port the capture server listens on. Send your requests here.
    #[arg(short = 'p', long, default_value_t = 9000)]
    pub port: u16,

    /// Port the web UI listens on.
    #[arg(short = 'u', long = "ui-port", default_value_t = 9001)]
    pub ui_port: u16,

    /// Bind address for both servers.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,

    /// Maximum captured body size in bytes. Larger bodies are truncated.
    #[arg(long, default_value_t = 10 * 1024 * 1024)]
    pub max_body_size: usize,

    /// Number of requests to keep in memory.
    #[arg(long, default_value_t = 1000)]
    pub buffer_size: usize,

    /// Disable the web UI server entirely.
    #[arg(long)]
    pub no_ui: bool,

    /// Disable the colored CLI output.
    #[arg(long)]
    pub no_cli: bool,

    /// Emit each request as JSON (NDJSON) to stdout.
    #[arg(long, conflicts_with = "no_cli")]
    pub json: bool,

    /// Open the web UI in your browser on startup.
    #[arg(long)]
    pub open: bool,

    /// Verbose CLI output: prints headers and a body preview for each request.
    #[arg(short, long)]
    pub verbose: bool,

    /// Download the latest release from GitHub and replace this binary, then exit.
    #[arg(long)]
    pub update: bool,

    /// Skip the startup check that asks GitHub if a newer release is available.
    #[arg(long)]
    pub no_update_check: bool,
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
        self.bind
            .parse::<IpAddr>()
            .map_err(|_| format!("invalid bind address: {}", self.bind))?;
        // Port 0 means "ephemeral, OS-assigned" so two zeroes resolve to two
        // different ports — only reject explicit clashes on a real port.
        if !self.no_ui && self.port != 0 && self.port == self.ui_port {
            return Err(format!(
                "capture port and UI port cannot both be {} — pass --ui-port or --no-ui",
                self.port
            ));
        }
        if self.max_body_size == 0 {
            return Err("--max-body-size must be > 0".into());
        }
        if self.buffer_size == 0 {
            return Err("--buffer-size must be > 0".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["postbin-ultra"];
        full.extend_from_slice(args);
        Cli::parse_from(full)
    }

    #[test]
    fn defaults_match_documented_values() {
        let cli = parse(&[]);
        assert_eq!(cli.port, 9000);
        assert_eq!(cli.ui_port, 9001);
        assert_eq!(cli.bind, "127.0.0.1");
        assert_eq!(cli.max_body_size, 10 * 1024 * 1024);
        assert_eq!(cli.buffer_size, 1000);
        assert!(!cli.no_ui);
        assert!(!cli.no_cli);
        assert!(!cli.json);
        assert!(!cli.open);
        assert!(!cli.verbose);
        cli.validate().unwrap();
    }

    #[test]
    fn parses_short_and_long_flags() {
        let cli = parse(&["-p", "8000", "-u", "8001", "-v"]);
        assert_eq!(cli.port, 8000);
        assert_eq!(cli.ui_port, 8001);
        assert!(cli.verbose);
    }

    #[test]
    fn parses_long_flags() {
        let cli = parse(&[
            "--port",
            "7000",
            "--ui-port",
            "7001",
            "--bind",
            "0.0.0.0",
            "--max-body-size",
            "2048",
            "--buffer-size",
            "10",
            "--no-ui",
        ]);
        assert_eq!(cli.port, 7000);
        assert_eq!(cli.ui_port, 7001);
        assert_eq!(cli.bind, "0.0.0.0");
        assert_eq!(cli.max_body_size, 2048);
        assert_eq!(cli.buffer_size, 10);
        assert!(cli.no_ui);
        cli.validate().unwrap();
    }

    #[test]
    fn validate_rejects_same_port_when_ui_enabled() {
        let cli = parse(&["-p", "9000", "-u", "9000"]);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("9000"));
    }

    #[test]
    fn validate_allows_same_port_when_ui_disabled() {
        let cli = parse(&["-p", "9000", "-u", "9000", "--no-ui"]);
        cli.validate().unwrap();
    }

    #[test]
    fn validate_rejects_invalid_bind() {
        let cli = parse(&["--bind", "not-an-ip"]);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("invalid"));
    }

    #[test]
    fn validate_rejects_zero_max_body() {
        let cli = parse(&["--max-body-size", "0"]);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("max-body-size"));
    }

    #[test]
    fn validate_rejects_zero_buffer() {
        let cli = parse(&["--buffer-size", "0"]);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("buffer-size"));
    }

    #[test]
    fn json_and_no_cli_conflict() {
        let res = Cli::try_parse_from(["postbin-ultra", "--json", "--no-cli"]);
        assert!(res.is_err());
    }

    #[test]
    fn cli_is_clone() {
        let cli = parse(&[]);
        let _cloned = cli.clone();
    }
}
