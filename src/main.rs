use clap::Parser;
use postbin_ultra::{
    app,
    cli::Cli,
    update::{self, UpdateOutcome},
};

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_tracing();

    if cli.update {
        return run_update();
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: failed to start tokio runtime: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    runtime.block_on(async {
        match app::run(cli).await {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e:#}");
                std::process::ExitCode::FAILURE
            }
        }
    })
}

fn run_update() -> std::process::ExitCode {
    match update::run_self_update() {
        Ok(UpdateOutcome::Updated { from, to }) => {
            println!("postbin-ultra updated from v{from} to v{to}");
            std::process::ExitCode::SUCCESS
        }
        Ok(UpdateOutcome::AlreadyLatest(v)) => {
            println!("postbin-ultra is already on the latest version (v{v})");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("update failed: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,postbin_ultra=info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
