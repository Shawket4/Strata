//! `stratad` entry point: parses arguments and dispatches (see the library docs).

use std::io::BufRead;
use std::process::ExitCode;

use clap::Parser;
use strata_common::Config;
use stratad::cli::{Cli, Command};
use stratad::commands::{self, CreateUser};

fn main() -> ExitCode {
    let cli = Cli::parse();
    stratad::logging::init();
    let config = match Config::load(cli.config.as_deref()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("stratad: {err}");
            return ExitCode::from(2);
        }
    };
    match run(cli.command, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!(error = %message, "stratad failed");
            eprintln!("stratad: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command, config: Config) -> Result<(), String> {
    match command {
        Command::Serve => actix_web::rt::System::new()
            .block_on(stratad::serve::run(config))
            .map_err(|e| e.to_string()),
        Command::Keygen { out, force } => {
            let path = out.unwrap_or_else(|| config.auth.signing_key_file.clone());
            commands::keygen(&path, force).map_err(|e| e.to_string())?;
            println!("wrote {}", path.display());
            Ok(())
        }
        Command::CreateUser {
            username,
            display_name,
            admin,
        } => {
            let mut password = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut password)
                .map_err(|e| e.to_string())?;
            let password = password.trim_end_matches(['\r', '\n']);
            let display_name = display_name.unwrap_or_else(|| username.clone());
            let id = runtime()?
                .block_on(commands::create_user(
                    &config,
                    &CreateUser {
                        username: &username,
                        display_name: &display_name,
                        password,
                        admin,
                    },
                ))
                .map_err(|e| e.to_string())?;
            println!("created {id}");
            Ok(())
        }
        Command::Verify { user } => {
            let report = runtime()?
                .block_on(commands::verify(&config, &user))
                .map_err(|e| e.to_string())?;
            let lines = [
                ("temp file", &report.temp_files_removed),
                ("uncommitted change", &report.recovered),
                ("note without an ID", &report.ids_assigned),
                ("sidecar to repair", &report.sidecars_repaired),
                ("note out of date in the index", &report.out_of_band),
                ("indexed note without a file", &report.missing),
            ];
            for (what, paths) in lines {
                for path in paths {
                    println!("{what}: {path}");
                }
            }
            if report.is_clean() {
                println!("vault is consistent");
                Ok(())
            } else {
                Err("the vault needs reconciliation (it runs when stratad serve starts)".into())
            }
        }
        Command::Reindex { user } => {
            let notes = runtime()?
                .block_on(commands::reindex(&config, &user))
                .map_err(|e| e.to_string())?;
            println!("reindexed {notes} notes");
            Ok(())
        }
        Command::Openapi { out } => {
            commands::openapi(&out).map_err(|e| e.to_string())?;
            println!("wrote {}", out.display());
            Ok(())
        }
        Command::Migrate => runtime()?
            .block_on(commands::migrate(&config))
            .map_err(|e| e.to_string()),
        Command::BootstrapRoles {
            print,
            superuser_url,
        } => {
            if print {
                print!(
                    "{}",
                    commands::bootstrap_script(&config).map_err(|e| e.to_string())?
                );
                return Ok(());
            }
            let url = superuser_url
                .ok_or("bootstrap-roles needs --superuser-url (or STRATA_SUPERUSER_URL)")?;
            runtime()?
                .block_on(commands::bootstrap_roles(&config, &url))
                .map_err(|e| e.to_string())
        }
    }
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())
}
