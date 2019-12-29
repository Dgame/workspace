use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use structopt::StructOpt;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Provider {
    Github,
}

fn git(args: &[&str], abs_path: Option<&Path>) -> Result<Output, std::io::Error> {
    if let Some(abs_path) = abs_path {
        Command::new("git")
            .current_dir(abs_path)
            .args(args)
            .output()
    } else {
        Command::new("git").args(args).output()
    }
}

struct Repository<'a> {
    local_path: PathBuf,
    git_path: &'a Path,
}

impl<'a> Repository<'a> {
    fn exists_local(&self) -> bool {
        self.local_path.exists()
    }
}

impl Provider {
    fn get_url(&self) -> &str {
        match *self {
            Self::Github => "https://github.com",
        }
    }

    fn git_pull<'a>(&self, repo: &Repository<'a>) {
        log::info!("- Pull {:?}...", repo.git_path);
        match *self {
            Self::Github => {
                git(&["pull"], Some(&repo.local_path)).expect("Failed to pull");
            }
        }
    }

    fn git_clone<'a>(&self, repo: &Repository<'a>) {
        let url = format!("{}/{}", self.get_url(), repo.git_path.display());
        log::info!("- Clone {}...", &url);
        match *self {
            Self::Github => {
                git(&["clone", &url], None).expect("Failed to clone");
            }
        }
    }

    fn git_fetch<'a>(&self, repo: &Repository<'a>) {
        log::info!("- Fetch {:?}...", repo.git_path);
        match *self {
            Self::Github => {
                git(&["fetch"], Some(&repo.local_path)).expect("Failed to fetch");
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Project {
    provider: Provider,
    path: PathBuf,
}

trait Git {
    fn git_pull(&self);
    fn git_clone(&self);
    fn git_fetch(&self);
    fn git_sync(&self);
}

impl Project {
    fn get_absolute_path(&self) -> PathBuf {
        use std::env;

        let folder = self.get_path().file_stem().expect("Could not get folder");

        env::current_dir()
            .expect("Could not get current path")
            .join(folder)
    }

    fn get_path(&self) -> &Path {
        &self.path
    }

    fn get_repository(&self) -> Repository {
        Repository {
            local_path: self.get_absolute_path(),
            git_path: self.get_path(),
        }
    }
}

impl Git for Project {
    fn git_pull(&self) {
        let repo = self.get_repository();
        if repo.exists_local() {
            self.provider.git_pull(&repo);
        } else {
            log::info!("~ {:?} is not cloned yet", repo.git_path);
        }
    }

    fn git_clone(&self) {
        let repo = self.get_repository();
        if !repo.exists_local() {
            self.provider.git_clone(&repo);
        } else {
            log::info!("~ {:?} is already cloned", repo.git_path);
        }
    }

    fn git_fetch(&self) {
        let repo = self.get_repository();
        if repo.exists_local() {
            self.provider.git_fetch(&repo);
        } else {
            log::info!("~ {:?} is not cloned yet", repo.git_path);
        }
    }

    fn git_sync(&self) {
        if self.get_repository().exists_local() {
            self.git_pull();
        } else {
            self.git_clone();
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Workspace {
    #[serde(default, rename = "workspace")]
    projects: Vec<Project>,
}

impl Git for Workspace {
    fn git_pull(&self) {
        log::info!("Pull...");
        self.projects.iter().for_each(|project| project.git_pull())
    }

    fn git_clone(&self) {
        log::info!("Clone...");
        self.projects.iter().for_each(|project| project.git_clone())
    }

    fn git_fetch(&self) {
        log::info!("Fetch...");
        self.projects.iter().for_each(|project| project.git_fetch())
    }

    fn git_sync(&self) {
        log::info!("Synchronize...");
        self.projects.iter().for_each(|project| project.git_sync());
    }
}

#[derive(StructOpt, Debug)]
enum Opt {
    #[structopt(name = "pull")]
    /// Pull all cloned repositories
    Pull,
    #[structopt(name = "clone")]
    /// Clone all not cloned repositories
    Clone,
    #[structopt(name = "fetch")]
    /// Fetch all cloned repositories
    Fetch,
    #[structopt(name = "sync")]
    /// Pull all cloned repositories, Clone all not cloned repositories
    Sync,
    #[structopt(name = "list")]
    /// List all workspace repositories
    List {
        #[structopt(long)]
        /// List only cloned workspace repositories
        cloned: bool,
    },
}

fn main() {
    use std::fs;
    use std::io::{BufReader, Read};

    simple_logger::init().expect("Could not init logger");

    let opt = Opt::from_args();
    if let Ok(file) = fs::File::open("workspace.toml") {
        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader
            .read_to_string(&mut contents)
            .expect("Could not read toml");

        let workspace: Workspace = toml::from_str(&contents).expect("Could not load Workspace");
        //dbg!(&workspace);
        match opt {
            Opt::Pull => workspace.git_pull(),
            Opt::Clone => workspace.git_clone(),
            Opt::Fetch => workspace.git_fetch(),
            Opt::Sync => workspace.git_sync(),
            Opt::List { cloned } => workspace.projects.iter().for_each(|project| {
                if cloned {
                    if project.get_repository().exists_local() {
                        println!(" - {}", project.path.display());
                    }
                } else {
                    println!(" - {}", project.path.display());
                }
            }),
        }
    } else {
        log::info!("That is not a valid workspace; missing workspace.toml");
    }
}
