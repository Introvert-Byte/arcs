use crate::archive::{ArchiveReader, ArchiveWriter};
use crate::cloud::CloudClient;
use crate::config::Profile;
use crate::discovery::discover_all;
use crate::error::{ArcsError, Result};
use crate::restore::{execute_restore, plan_restore, RestoreOptions};
use crate::util::{current_os_arch, human_size};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "arcs", version, about = "Cross-platform backup & migration utility")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Launch desktop GUI (default when no subcommand)
    #[arg(long)]
    pub gui: bool,

    /// Launch interactive terminal UI
    #[arg(long)]
    pub tui: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Discover apps and configs on this system
    Discover {
        #[arg(long)]
        json: bool,
    },

    /// Create a new .arcs archive
    Backup {
        /// Output path (.arcs)
        #[arg(short, long)]
        output: PathBuf,

        /// Paths to include
        #[arg(short, long)]
        path: Vec<PathBuf>,

        /// Encrypt with password (prompt if omitted and --encrypt)
        #[arg(long)]
        password: Option<String>,

        #[arg(long)]
        encrypt: bool,

        #[arg(long)]
        comment: Option<String>,
    },

    /// Inspect an .arcs archive
    Inspect {
        file: PathBuf,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Restore from an .arcs archive
    Restore {
        file: PathBuf,
        #[arg(long)]
        target: Option<PathBuf>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        overwrite: bool,
    },

    /// Search inside an archive
    Search {
        file: PathBuf,
        query: String,
        #[arg(long)]
        password: Option<String>,
    },

    /// Diff two archives
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long)]
        password: Option<String>,
    },

    /// Run a named profile (YAML/JSON)
    Run {
        profile: PathBuf,
    },

    /// Push archive to cloud
    Push {
        file: PathBuf,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        bucket: String,
        #[arg(long)]
        access_key: Option<String>,
        #[arg(long)]
        secret_key: Option<String>,
    },

    /// Pull archive from cloud
    Pull {
        name: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        bucket: String,
        #[arg(long)]
        access_key: Option<String>,
        #[arg(long)]
        secret_key: Option<String>,
    },
}

pub async fn run_cli(cli: Cli) -> Result<()> {
    // No subcommand → GUI by default (or explicit flags)
    if cli.command.is_none() {
        if cli.tui {
            return crate::tui::run();
        }
        // GUI is the default app experience
        crate::gui::run_gui().map_err(|e| ArcsError::Other(e.to_string()))?;
        return Ok(());
    }
    if cli.gui {
        crate::gui::run_gui().map_err(|e| ArcsError::Other(e.to_string()))?;
        return Ok(());
    }
    if cli.tui {
        return crate::tui::run();
    }

    match cli.command {
        Some(Commands::Discover { json }) => {
            let items = discover_all()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                println!("Discovered {} items:\n", items.len());
                for item in &items {
                    println!(
                        "  [{:}] {:<30} ({:?})  {}",
                        if item.selected { "x" } else { " " },
                        item.name,
                        item.kind,
                        item.path.display()
                    );
                }
            }
            Ok(())
        }
        Some(Commands::Backup {
            output,
            path,
            password,
            encrypt,
            comment,
        }) => {
            let (os, arch) = current_os_arch();
            let mut writer = ArchiveWriter::new(&os, &arch);
            if let Some(c) = comment {
                writer.set_comment(c);
            }
            if encrypt || password.is_some() {
                let pw = password.unwrap_or_else(|| {
                    dialoguer::Password::new()
                        .with_prompt("Encryption password")
                        .interact()
                        .unwrap_or_default()
                });
                writer.set_password(pw);
            }
            for p in path {
                println!("Adding {} …", p.display());
                writer.add_path(&p, "")?;
            }
            writer.finish(&output)?;
            println!("Wrote {}", output.display());
            Ok(())
        }
        Some(Commands::Inspect {
            file,
            password,
            json,
        }) => {
            let reader = ArchiveReader::open(&file)?;
            let manifest = reader.manifest(password.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!("Archive : {}", file.display());
                println!("ID      : {}", manifest.id);
                println!("Created : {}", manifest.created_at);
                println!("Source  : {} / {}", manifest.source_os, manifest.source_arch);
                println!("Encrypted: {}", manifest.encrypted);
                println!("Files   : {}", manifest.file_count());
                println!("Size    : {}", human_size(manifest.total_size()));
                println!("Packages: {}", manifest.packages.len());
                if let Some(c) = &manifest.comment {
                    println!("Comment : {}", c);
                }
                println!("\nEntries:");
                for e in manifest.entries.iter().take(50) {
                    println!(
                        "  {:>10}  {}",
                        human_size(e.size),
                        e.archive_path
                    );
                }
                if manifest.entries.len() > 50 {
                    println!("  … and {} more", manifest.entries.len() - 50);
                }
            }
            Ok(())
        }
        Some(Commands::Restore {
            file,
            target,
            password,
            dry_run,
            overwrite,
        }) => {
            let reader = ArchiveReader::open(&file)?;
            let opts = RestoreOptions {
                target_root: target.unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| ".".into())),
                dry_run,
                password,
                selected_paths: None,
                overwrite,
            };
            let plan = plan_restore(&reader, &opts)?;
            for w in &plan.warnings {
                eprintln!("warning: {}", w);
            }
            execute_restore(&reader, &plan, &opts)?;
            if dry_run {
                println!("Dry-run complete ({} actions)", plan.actions.len());
            } else {
                println!("Restore finished");
            }
            Ok(())
        }
        Some(Commands::Search {
            file,
            query,
            password,
        }) => {
            let reader = ArchiveReader::open(&file)?;
            let manifest = reader.manifest(password.as_deref())?;
            let q = query.to_lowercase();
            let mut found = 0;
            for e in &manifest.entries {
                if e.archive_path.to_lowercase().contains(&q)
                    || e.original_path.to_lowercase().contains(&q)
                {
                    println!("{}  ({})", e.archive_path, human_size(e.size));
                    found += 1;
                }
            }
            for p in &manifest.packages {
                if p.name.to_lowercase().contains(&q) {
                    println!("[pkg] {} ({})", p.name, p.manager);
                    found += 1;
                }
            }
            println!("\n{} match(es)", found);
            Ok(())
        }
        Some(Commands::Diff { a, b, password }) => {
            let ra = ArchiveReader::open(&a)?;
            let rb = ArchiveReader::open(&b)?;
            let ma = ra.manifest(password.as_deref())?;
            let mb = rb.manifest(password.as_deref())?;
            let set_a: std::collections::HashSet<_> =
                ma.entries.iter().map(|e| e.archive_path.as_str()).collect();
            let set_b: std::collections::HashSet<_> =
                mb.entries.iter().map(|e| e.archive_path.as_str()).collect();
            println!("Only in {}:", a.display());
            for p in set_a.difference(&set_b) {
                println!("  + {}", p);
            }
            println!("Only in {}:", b.display());
            for p in set_b.difference(&set_a) {
                println!("  - {}", p);
            }
            Ok(())
        }
        Some(Commands::Run { profile }) => {
            let profile = Profile::load(&profile)?;
            println!("Running profile: {}", profile.name);
            let (os, arch) = current_os_arch();
            let mut writer = ArchiveWriter::new(&os, &arch);
            if profile.encrypt {
                if let Some(ref pw) = profile.password {
                    writer.set_password(pw.clone());
                } else {
                    let pw = dialoguer::Password::new()
                        .with_prompt("Encryption password")
                        .interact()
                        .map_err(|e| ArcsError::Other(e.to_string()))?;
                    writer.set_password(pw);
                }
            }
            for inc in &profile.include {
                let p = PathBuf::from(inc);
                if p.exists() {
                    writer.add_path(&p, "")?;
                }
            }
            let out = profile.resolve_output();
            if profile.dry_run {
                println!("[dry-run] would write {}", out.display());
            } else {
                writer.finish(&out)?;
                println!("Wrote {}", out.display());
                if let Some(cloud) = profile.cloud {
                    let client = CloudClient::new();
                    client.push(&out, &cloud).await?;
                    println!("Pushed to cloud");
                }
            }
            Ok(())
        }
        Some(Commands::Push {
            file,
            provider,
            endpoint,
            bucket,
            access_key,
            secret_key,
        }) => {
            let target = crate::config::CloudTarget {
                provider: match provider.to_lowercase().as_str() {
                    "s3" => crate::config::CloudProvider::S3,
                    "webdav" => crate::config::CloudProvider::WebDav,
                    "local" => crate::config::CloudProvider::Local,
                    _ => return Err(ArcsError::Cloud("unknown provider".into())),
                },
                endpoint,
                bucket_or_path: bucket,
                access_key,
                secret_key,
            };
            CloudClient::new().push(&file, &target).await?;
            println!("Pushed {}", file.display());
            Ok(())
        }
        Some(Commands::Pull {
            name,
            output,
            provider,
            endpoint,
            bucket,
            access_key,
            secret_key,
        }) => {
            let target = crate::config::CloudTarget {
                provider: match provider.to_lowercase().as_str() {
                    "s3" => crate::config::CloudProvider::S3,
                    "webdav" => crate::config::CloudProvider::WebDav,
                    "local" => crate::config::CloudProvider::Local,
                    _ => return Err(ArcsError::Cloud("unknown provider".into())),
                },
                endpoint,
                bucket_or_path: bucket,
                access_key,
                secret_key,
            };
            CloudClient::new().pull(&name, &output, &target).await?;
            println!("Pulled → {}", output.display());
            Ok(())
        }
        None => Ok(()), // handled above
    }
}