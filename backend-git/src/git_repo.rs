use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::error::Error;
use std::collections::HashMap;
use std::sync::RwLock;

use gpp_core::types::{NodeId, RemoteRef, Author};
use gpp_core::backend::RepoBackend;

/// Конкретная реализация `RepoBackend`, использующая утилиту CLI `git`.
pub struct GitRepo {
    workdir: PathBuf,
    backend_cfg: HashMap<String, String>,
    active_remote: RwLock<Option<String>>,
}

impl GitRepo {
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        GitRepo {
            workdir: workdir.as_ref().to_path_buf(),
            backend_cfg: HashMap::new(),
            active_remote: RwLock::new(None),
        }
    }

    /// Инициализация bare-репозитория или рабочей копии для изоляции.
    pub fn init_bare(&self, dir_name: &str) -> Result<(), Box<dyn Error>> {
        let git_dir = self.workdir.join(dir_name);

        if git_dir.exists() {
            return Ok(());
        }

        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
        command.arg("init");

        if dir_name == ".git" {
            command.output()?;
        } else {
            command.arg("--separate-git-dir").arg(dir_name);
            command.output()?;
            let git_link_file = self.workdir.join(".git");
            if git_link_file.is_file() {
                std::fs::remove_file(git_link_file).ok();
            }
        }
        Ok(())
    }

    /// Вспомогательный метод для запуска команд.
    fn run_git_command(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);

        let remote_name_opt = self.active_remote.read().unwrap().clone();
        let git_dir_name = match remote_name_opt {
            Some(ref name) => format!(".git_{}", name),
            None => ".git".to_string(),
        };

        let git_dir_path = self.workdir.join(&git_dir_name);
        command.env("GIT_DIR", &git_dir_path);
        command.env("GIT_WORK_TREE", &self.workdir);

        command.args(args);

        let output = command.output()?;

        if !output.status.success() {
            let error_msg = format!(
                "Git error (ctx={}) cmd='{:?}': {}",
                git_dir_name,
                args,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Err(error_msg.into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn switch_context(&self, remote_name: Option<&str>) -> Result<(), Box<dyn Error>> {
        if let Some(name) = remote_name {
            let dir_name = format!(".git_{}", name);
            self.init_bare(&dir_name)?;
        }
        let mut lock = self.active_remote.write().unwrap();
        *lock = remote_name.map(|s| s.to_string());
        Ok(())
    }
}

impl RepoBackend for GitRepo {
    fn run_cmd(&self, cmd: &str, args: Vec<&str>) -> Result<Output, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);

        let remote_name_opt = self.active_remote.read().unwrap().clone();
        let git_dir_name = match remote_name_opt {
            Some(ref name) => format!(".git_{}", name),
            None => ".git".to_string(),
        };

        command.env("GIT_DIR", self.workdir.join(git_dir_name));
        command.env("GIT_WORK_TREE", &self.workdir);

        command.arg(cmd);
        command.args(&args);

        Ok(command.output()?)
    }

    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>> {
        let args = vec!["rev-parse", "--verify", &refname];
        match self.run_git_command(&args) {
            Ok(hash) => Ok(Some(NodeId(hash))),
            Err(_) => Ok(None),
        }
    }

    fn create_tree(&self) -> Result<String, Box<dyn Error>> {
        self.run_git_command(&vec!["add", "-A"])?;
        let tree_hash = self.run_git_command(&vec!["write-tree"])?;
        Ok(tree_hash)
    }

    fn create_commit(
        &self,
        tree_oid: &str,
        parents: &[NodeId],
        message: &str,
        author: &Author
    ) -> Result<NodeId, Box<dyn Error>> {
        let mut args = vec!["commit-tree", tree_oid, "-m", message];
        for p in parents {
            args.push("-p");
            args.push(&p.0);
        }
        let commit_hash = self.run_git_command(&args)?;
        Ok(NodeId(commit_hash))
    }

    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>> {
        self.switch_context(Some(&remote.name))?;
        self.switch_context(None)?;

        let refspec = format!("{}:{}", local_tip_id.0, remote_target_ref);
        let args = vec!["push", &remote.url, &refspec];
        self.run_git_command(&args)?;
        Ok(())
    }
}