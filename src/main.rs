use std::path::{Path, PathBuf};

use clap::{command, Parser, Subcommand};
use miette::{Diagnostic, Result};
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
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Adds a dotfile to the dotfiles and links it back to its original place
    Add { file: PathBuf },
    /// Remove a dotfile and any potential symlink, can be pointed both at the symlink and the original
    Remove { file: PathBuf },
    /// Links or relinks all dotfiles
    Link,
    /// Lists all dotfiles
    List,
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
    ListDirectoryFailed(#[from] walkdir::Error)
}

fn main() -> miette::Result<()> {
    let args = Args::parse();

    let base_directory = args.base_directory.canonicalize().map_err(|e| DofiError::InvalidBaseDirectory(e, args.base_directory))?;
    let dotfiles_directory = args.dotfiles_directory.canonicalize().map_err(|e| DofiError::InvalidDotfilesDirectory(e, args.dotfiles_directory))?;

    match args.command {
        Commands::Add { file } => {
            let file = resolve_file(file)?;
            add_file(&file, &base_directory, &dotfiles_directory)
        }
        Commands::Link => {
            link_files(&base_directory, &dotfiles_directory)
        },
        Commands::List => {
            list_files(&dotfiles_directory)
        },
        Commands::Remove { file } => {
            let file = resolve_file(file)?;
            remove_file(&file, &base_directory, &dotfiles_directory)
        }
    }?;

    Ok(())
}

fn resolve_file(file: PathBuf) -> Result<PathBuf, DofiError> {
    if file.is_symlink() || !file.is_file() {
        return Err(DofiError::FileIsNotRegular(file.to_path_buf()))
    }
    
    let file = file.canonicalize().map_err(DofiError::GenericIoError)?;

    Ok(file)
}

fn remove_file(file: &Path, base: &Path, dotfiles: &Path) -> Result<(), DofiError> {
    todo!()
}

fn add_file(file: &Path, base_directory: &Path, dotfiles_directory: &Path) -> Result<(), DofiError> {
    let new_file = file
        .strip_prefix(base_directory)
        .map(|relative_file| dotfiles_directory.join(relative_file))
        .map_err(|_| DofiError::BaseIsNotPrefixOfFile(base_directory.to_path_buf(), file.to_path_buf()))?;

    if let Some(parent) = new_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(file, &new_file)?;
    std::os::unix::fs::symlink(&new_file, file)?;

    Ok(())
}

fn link_files<P>(base_directory: P, dotfiles_directory: P) -> Result<(), DofiError> where P: AsRef<Path> {
    todo!()
}

fn list_files(dotfiles_directory: &Path) -> Result<(), DofiError> {
    let walker = WalkDir::new(dotfiles_directory).into_iter().filter(|e| {
        match e {
            Ok(e) => e.file_type().is_file(),
            _ => true
        }
    });

    for entry in walker {
        println!("{}", entry?.path().display());
    }

    Ok(())
}
