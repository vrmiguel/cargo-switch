use std::env;
use std::fs;
use std::fs::read_dir;
use std::io;
use std::io::BufRead;
use std::ops::Not;
use std::os::unix;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo-switch")]
#[command(about = "Manage multiple versions of Cargo binaries", long_about = None)]
struct Cli {
    #[arg(value_name = "PACKAGE@VERSION", required = false)]
    package_version: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install {
        #[arg(value_name = "PACKAGE")]
        package: String,
    },
    List,
}

pub struct Switcher {
    registry: PathBuf,
}

impl Switcher {
    fn get_cargo_bin() -> Result<PathBuf> {
        let path = env::var("PATH").expect("failed to find PATH");

        path
            .split(':')
            .find(|component| component.contains(".cargo/bin"))
            .map(Path::new)
            .with_context(|| {
                "Failed to find your .cargo/bin directory. Is Cargo configured in your PATH?"
            }).map(ToOwned::to_owned)
    }

    pub fn new() -> Result<Self> {
        let cargo_path = Self::get_cargo_bin()?;

        ensure!(
            cargo_path.exists(),
            ".cargo/bin directory in $PATH does not exist"
        );

        let switch_path = cargo_path.join("cargo-switch-registry");
        if switch_path.exists().not() {
            fs::create_dir(&switch_path).unwrap()
        }

        Ok(Self {
            registry: switch_path,
        })
    }

    /// Perform some basic input checking and return the project name and version. Expects input to be in the
    /// `name@semver` format.
    fn get_version_tag(package: &str) -> Option<(&str, &str)> {
        let (project_name, version) = package.split_once('@')?;

        let good_enough = project_name.len() >= 1 && version.chars().any(|ch| ch.is_ascii_digit());

        good_enough.then(|| (project_name, version))
    }

    fn build_target_path(&self, package: &str) -> Result<PathBuf> {
        let (project_name, project_version) = Self::get_version_tag(package)
            .with_context(|| "Expected input in the form `NAME@VERSION`")?;

        Ok(self.registry.join(project_name).join(project_version))
    }

    pub fn install_package(&self, package: &str) -> Result<()> {
        let target_path = self.build_target_path(package)?;

        let mut child = Command::new("cargo")
            .arg("install")
            .arg(package)
            .arg("--root")
            .arg(target_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to execute cargo install");

        let stderr = child.stderr.take().expect("Failed to capture stderr");
        let reader = io::BufReader::new(stderr);

        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
            }
        }

        let status = child.wait().expect("Failed to wait on child process");

        if status.success() {
            println!("Successfully installed {}", package);
        } else {
            eprintln!("Failed to install {}", package);
        }

        self.switch_package(package)?;

        Ok(())
    }

    fn list_packages(&self) -> Result<()> {
        let readdir = fs::read_dir(&self.registry)?;

        for maybe_entry in readdir {
            let entry = match maybe_entry {
                Ok(entry) => entry,
                Err(err) => {
                    eprintln!("{err}");
                    continue;
                },
            };
            let entry_path = entry.path();
            // Should be a safe unwrap
            let project_name = entry_path.components().last().unwrap().as_os_str();
            println!("{}:", Path::new(project_name).display());

            // Read dir again to fetch versions
            let inner_readdir = fs::read_dir(&entry_path)?;
            for maybe_entry in inner_readdir {
                let entry = maybe_entry?;
                let entry_path = entry.path();
                let project_version = entry_path.components().last().unwrap().as_os_str();
                println!("  - {}", Path::new(project_version).display());
            }
        }

        Ok(())
    }

    fn switch_package(&self, package: &str) -> Result<()> {
        let switch_registry = self.build_target_path(package)?;
        let cargo_bin = Self::get_cargo_bin()?;
    
        ensure!(switch_registry.exists(), "Project {package} is not installed!");

        let project_bin = switch_registry.join("bin");
        ensure!(switch_registry.exists(), "Expected {} to exist", project_bin.display());

        for maybe_entry in read_dir(project_bin)? {
            let entry = maybe_entry?;
            let entry_path = entry.path();

            // Assumes every binary will be in the form `$CARGO_BIN/bin/binary`. If it has subdirectories and such,
            // I expect this logic to fail
            let file_name = entry_path.components().last().unwrap().as_os_str();
            let symlink_path = cargo_bin.join(file_name);
            if symlink_path.exists() {
                fs::remove_file(&symlink_path)?;
            }

            unix::fs::symlink(&entry_path, &symlink_path)?;
            println!("Linked {} to {}", entry_path.display(), symlink_path.display());
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let switcher = Switcher::new()?;

    if let Some(package_version) = &cli.package_version {
        switcher.switch_package(package_version)?;
    } else if let Some(command) = &cli.command {
        match command {
            Commands::Install { package } => {
                switcher.install_package(package)?;
            }
            Commands::List => {
                switcher.list_packages()?;
            }
        }
    } else {
        eprintln!("No command or package version specified. Use --help for more information.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Switcher;

    #[test]
    fn has_version_tag() {
        assert!(Switcher::get_version_tag("sqlx-cli@0.7.2").is_some());
        assert!(Switcher::get_version_tag("zig@1.0.0-rc0").is_some());

        assert!(Switcher::get_version_tag("zig@rc").is_none());
        assert!(Switcher::get_version_tag("zig@").is_none());
        assert!(Switcher::get_version_tag("@0.7.2").is_none());
    }
}
