use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Provider {
    Github,
}

fn git(args: &[&str], abs_path: Option<&Path>) -> std::io::Result<Output> {
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
    fn from(provider: &str) -> Option<Self> {
        match provider {
            "github" => Some(Self::Github),
            "github.com" => Some(Self::Github),
            _ => None,
        }
    }

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
    #[serde(default)]
    cmd: Vec<String>,
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

    fn build(&self) {
        match self.cmd.len() {
            0 => Ok(()),
            1 => Command::new(&self.cmd[0])
                .current_dir(self.get_absolute_path())
                .output()
                .and_then(|_| Ok(())),
            _ => Command::new(&self.cmd[0])
                .current_dir(self.get_absolute_path())
                .args(&self.cmd[1..])
                .output()
                .and_then(|_| Ok(())),
        }
        .expect("Could not build");
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

impl Workspace {
    fn build(&self) {
        log::info!("Build...");
        self.projects.iter().for_each(|project| project.build())
    }

    fn save(&mut self) {
        use std::fs;

        fs::write(
            "workspace.toml",
            toml::to_string(&self).expect("Failed save workspace.toml"),
        )
        .expect("Unable to write file");
    }

    fn add(&mut self, path: &Path, cmd: Option<String>) -> std::io::Result<()> {
        use std::env;

        let current_dir = env::current_dir()?;
        let git_path = current_dir.join(path).join(".git");
        if git_path.exists() {
            if let Ok(output) = git(&["config", "--get", "remote.origin.url"], Some(path)) {
                let remote_url = String::from_utf8_lossy(&output.stdout);
                if let Ok(url) = url::Url::parse(&remote_url) {
                    if let Some(host) = url.host_str() {
                        if let Some(provider) = Provider::from(host) {
                            let cmd = if let Some(cmd) = cmd {
                                cmd.split(' ').map(|s| s.to_string()).collect()
                            } else {
                                Vec::new()
                            };

                            let path = PathBuf::from(url.path().trim_start_matches('/'));
                            if self
                                .projects
                                .iter()
                                .position(|p| p.path == path && p.provider == provider)
                                .is_none()
                            {
                                let project = Project {
                                    provider,
                                    path,
                                    cmd,
                                };
                                log::info!(
                                    "Path {:?} with provider {:?}",
                                    project.path,
                                    project.provider
                                );
                                self.projects.push(project);
                            }
                        } else {
                            log::error!("Could not identify provider for {:?}", host);
                        }
                    } else {
                        log::error!(
                            "Invalid remote-url {:?}. Could not determine host.",
                            remote_url
                        );
                    }
                } else {
                    log::error!("Could not parse url {:?}", remote_url);
                }
            } else {
                log::error!("Invalid remote for {:?}", path);
            }
        } else {
            log::error!("{:?} is not a git repository", path);
        }

        Ok(())
    }

    fn remove(&mut self, path: &Path, provider: Provider) {
        if let Some(index) = self
            .projects
            .iter()
            .position(|p| p.path == path && p.provider == provider)
        {
            self.projects.remove(index);
            log::info!("Path {:?} with provider {:?} was removed", path, provider);
        }
    }

    fn scan(&mut self, path: Option<PathBuf>) -> std::io::Result<()> {
        use std::env;
        use std::fs;

        let current_dir = env::current_dir()?;
        let path = path.map_or(current_dir.clone(), |path| current_dir.join(path));

        log::info!("Scanning {:?}...", path);

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::metadata(&path)?;

            if !metadata.is_file() {
                self.add(&path, None).ok();
            }
        }

        Ok(())
    }
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
    #[structopt(name = "build")]
    /// Build all cloned repositories
    Build,
    #[structopt(name = "add")]
    /// Add a new repository
    Add {
        #[structopt(long)]
        /// Path of the repository
        path: PathBuf,
        #[structopt(long)]
        /// Optional build command for the repository
        cmd: Option<String>,
    },
    #[structopt(name = "rm")]
    /// Remove an existing repository
    Remove {
        #[structopt(long)]
        /// Path of the repository
        path: PathBuf,
        #[structopt(long)]
        /// Provider of the repository
        provider: String,
    },
    #[structopt(name = "scan")]
    /// Scan for repositories and add them to the workspace
    Scan {
        #[structopt(long)]
        /// Optional path which should be scanned, default to current directory
        path: Option<PathBuf>,
    },
}

fn main() {
    use std::fs;

    simple_logger::init().expect("Could not init logger");

    let opt = Opt::from_args();
    if let Ok(content) = fs::read("workspace.toml") {
        let mut workspace: Workspace =
            toml::from_str(&String::from_utf8_lossy(&content)).expect("Could not load Workspace");
        //dbg!(&workspace);
        match opt {
            Opt::Pull => workspace.git_pull(),
            Opt::Clone => workspace.git_clone(),
            Opt::Fetch => workspace.git_fetch(),
            Opt::Sync => workspace.git_sync(),
            Opt::List { cloned } => workspace.projects.iter().for_each(|project| {
                if cloned {
                    if project.get_repository().exists_local() {
                        log::info!(" - {}", project.path.display());
                    }
                } else {
                    log::info!(" - {}", project.path.display());
                }
            }),
            Opt::Build => workspace.build(),
            Opt::Add { path, cmd } => {
                workspace.add(&path, cmd).ok();
                workspace.save();
            }
            Opt::Remove { path, provider } => {
                if let Some(provider) = Provider::from(&provider) {
                    workspace.remove(&path, provider);
                    workspace.save();
                } else {
                    log::error!("Invalid provider: {}", provider);
                }
            }
            Opt::Scan { path } => {
                workspace.scan(path).ok();
                workspace.save();
            }
        }
    } else {
        log::info!("That is not a valid workspace; missing workspace.toml");
    }
}
