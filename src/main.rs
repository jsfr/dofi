use std::{io, path::{Path, PathBuf}};

use clap::{command, Command, CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Generator, Shell};
use log::info;
use miette::{bail, Diagnostic, Result};
use thiserror::Error;
use walkdir::WalkDir;

/// A simple dotfile manager, inspired by stow
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, env = "DOFI_DIR")]
    dotfiles_directory: PathBuf,

    #[arg(short, env = "HOME")]
    base_directory: PathBuf,

    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Adds a dotfile to the dotfiles and links it back to its original place
    Add { file: PathBuf },
    /// Remove a dotfile and any potential symlink, can be pointed both at the symlink and the original
    #[command(alias = "rm")]
    Remove { file: PathBuf },
    /// Links or relinks all dotfiles
    #[command(alias = "ln")]
    Link {
        #[arg(short, long, default_value_t = false)]
        force: bool
    },
    /// Lists all dotfiles
    #[command(alias = "ls")]
    List,
    /// Generate shell completions
    Completions { shell: Shell }
}

#[derive(Error, Diagnostic, Debug)]
enum DofiError {
    #[error(transparent)]
    #[diagnostic(code(dofi::io_error))]
    GenericIoError(#[from] std::io::Error),

    #[error("Base '{}' is not a prefix of target '{}'", .0.display(), .1.display())]
    #[diagnostic(code(dofi::prefix_error))]
    BaseIsNotPrefixOfFile(PathBuf, PathBuf),

    #[error("Target '{}' is not a regular file", .0.display())]
    #[diagnostic(code(dofi::not_regular_file_error))]
    FileIsNotRegular(PathBuf),

    #[error("Invalid base directory '{}': {0}", .1.display())]
    #[diagnostic(code(dofi::base_dir_error))]
    InvalidBaseDirectory(std::io::Error, PathBuf),

    #[error("Invalid dotfiles directory '{}': {0}", .1.display())]
    #[diagnostic(code(dofi::dotfiles_dir_error))]
    InvalidDotfilesDirectory(std::io::Error, PathBuf),
    
    #[error(transparent)]
    #[diagnostic(code(dofi::walkdir_error))]
    ListDirectoryFailed(#[from] walkdir::Error),

    #[error("File '{}' is not a dotfile", .0.display())]
    #[diagnostic(code(dofi::file_is_not_a_dotfile))]
    FileIsNotADotfile(PathBuf)
}

fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    let base_directory = args.base_directory.canonicalize().map_err(|e| DofiError::InvalidBaseDirectory(e, args.base_directory))?;
    let dotfiles_directory = args.dotfiles_directory.canonicalize().map_err(|e| DofiError::InvalidDotfilesDirectory(e, args.dotfiles_directory))?;

    match args.command {
        Commands::Add { file } => {
            if file.is_symlink() || !file.is_file() {
                bail!(DofiError::FileIsNotRegular(file.to_path_buf()))
            }
            let file = file.canonicalize().map_err(DofiError::GenericIoError)?;
            add_file(&file, &base_directory, &dotfiles_directory)?;
        }
        Commands::Link { force } => {
            link_files(&base_directory, &dotfiles_directory, force)?;
        },
        Commands::List => {
            list_files(&dotfiles_directory)?;
        },
        Commands::Remove { file } => {
            if !file.is_file() {
                bail!(DofiError::FileIsNotRegular(file.to_path_buf()))
            }
            let file = file.canonicalize().map_err(DofiError::GenericIoError)?;
            remove_file(&file, &base_directory, &dotfiles_directory)?;
        },
        Commands::Completions { shell } => {
            let mut cmd = Args::command();
            print_completions(shell, &mut cmd);
        }
            
    };

    Ok(())
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn remove_file(file: &Path, base_directory: &Path, dotfiles_directory: &Path) -> Result<(), DofiError> {
    if !file.starts_with(dotfiles_directory) {
        return Err(DofiError::FileIsNotADotfile(file.to_path_buf()))
    }

    info!("Removing file '{}'", file.display());
    std::fs::remove_file(file)?;

    let symlink = file.strip_prefix(dotfiles_directory).map(|relative_file| base_directory.join(relative_file)).map_err(|_| DofiError::BaseIsNotPrefixOfFile(base_directory.to_path_buf(), file.to_path_buf()))?;

    if symlink.symlink_metadata().is_ok() {
        info!("Removing symlink '{}'", symlink.display());
        let _ = std::fs::remove_file(symlink);
    }

    Ok(())
}

fn add_file(file: &Path, base_directory: &Path, dotfiles_directory: &Path) -> Result<(), DofiError> {
    let new_file = file
        .strip_prefix(base_directory)
        .map(|relative_file| dotfiles_directory.join(relative_file))
        .map_err(|_| DofiError::BaseIsNotPrefixOfFile(base_directory.to_path_buf(), file.to_path_buf()))?;

    if let Some(parent) = new_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("Moving '{}' to '{}'", file.display(), new_file.display());
    std::fs::rename(file, &new_file)?;
    info!("Symlinking '{}' at '{}'", new_file.display(), file.display());
    std::os::unix::fs::symlink(&new_file, file)?;

    Ok(())
}

fn link_files(base_directory: &Path, dotfiles_directory: &Path, force: bool) -> Result<(), DofiError> {
    let walker = WalkDir::new(dotfiles_directory).into_iter().filter(|e| {
        if let Ok(e) = e {
            e.file_type().is_file()
        } else {
            true
        }
    });

    for entry in walker {
        let file = entry?;

        let symlink = file
            .path()
            .strip_prefix(dotfiles_directory)
            .map(|relative_file| base_directory.join(relative_file))
            .map_err(|_| DofiError::BaseIsNotPrefixOfFile(base_directory.to_path_buf(), file.path().to_path_buf()))?;

        if let Some(parent) = symlink.parent() {
            info!("Create folder '{}'", parent.display());
            std::fs::create_dir_all(parent)?;
        }

        if force && symlink.symlink_metadata().is_ok() {
            info!("Removing existing file '{}'", symlink.display());
            std::fs::remove_file(&symlink)?;
        }

        info!("Symlinking '{}' at '{}'", file.path().display(), symlink.display());
        std::os::unix::fs::symlink(file.path(), &symlink)?
    }

    Ok(())
}

fn list_files(dotfiles_directory: &Path) -> Result<(), DofiError> {
    let walker = WalkDir::new(dotfiles_directory).into_iter().filter(|e| {
        if let Ok(e) = e {
            e.file_type().is_file()
        } else {
            true
        }
    });

    for entry in walker {
        println!("{}", entry?.path().display());
    }

    Ok(())
}
