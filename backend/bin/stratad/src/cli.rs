//! Command-line interface.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The Strata server.
#[derive(Debug, Parser)]
#[command(name = "stratad", version, about)]
pub struct Cli {
    /// Configuration file (`stratad.toml`); `STRATA__…` environment variables override it.
    #[arg(long, short, global = true, env = "STRATA_CONFIG")]
    pub config: Option<PathBuf>,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the API server (after checking secrets, the key and the database locale).
    Serve,
    /// Generate the Ed25519 access-token signing key (PKCS#8 PEM, mode 0600).
    Keygen {
        /// Where to write it (default: `auth.signing_key_file`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Replace an existing key (signs every device out within one access-token lifetime).
        #[arg(long)]
        force: bool,
    },
    /// Create an active account with its vault; the password is read from standard input.
    CreateUser {
        /// Username.
        #[arg(long)]
        username: String,
        /// Display name (default: the username).
        #[arg(long)]
        display_name: Option<String>,
        /// Make the account an admin.
        #[arg(long)]
        admin: bool,
    },
    /// Check a user's vault against its git history and the index, changing nothing
    /// (exit status 1 if anything is wrong). Run with the server stopped.
    Verify {
        /// User ID or username.
        #[arg(long)]
        user: String,
    },
    /// Rebuild every derived index row of a user from the vault files (after reconciling
    /// the vault). Run with the server stopped.
    Reindex {
        /// User ID or username.
        #[arg(long)]
        user: String,
    },
    /// Write the OpenAPI contract.
    Openapi {
        /// Output path.
        #[arg(long, default_value = "api/openapi.json")]
        out: PathBuf,
    },
    /// Apply database migrations (as `strata_owner`).
    Migrate,
    /// Create the database roles and grants (as a superuser), or print the SQL.
    BootstrapRoles {
        /// Print the script instead of applying it.
        #[arg(long)]
        print: bool,
        /// Superuser connection URL (any database of the cluster).
        #[arg(long, env = "STRATA_SUPERUSER_URL")]
        superuser_url: Option<String>,
    },
}
