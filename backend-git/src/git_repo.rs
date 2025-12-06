use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::error::Error;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir as symlink;

use gpp_core::types::{NodeId, RemoteRef, Author};
use gpp_core::backend::RepoBackend;

pub struct GitRepo {
    workdir: PathBuf,
}

impl GitRepo {
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        Self {
            workdir: workdir.as_ref().to_path_buf(),
        }
    }

    /// Вспомогательный метод для запуска git команд
    fn run_git_command(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
        command.env("GIT_DIR", self.workdir.join(".git"));
        command.env("GIT_WORK_TREE", &self.workdir);
        // command.env("GIT_CONFIG_NOSYSTEM", "1");
        command.args(args);

        let output = command.output()?;

        if !output.status.success() {
            let error_msg = format!(
                "Git error cmd='git {:?}': {}",
                args,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Err(error_msg.into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn switch_context(&self, remote_name: &str) -> Result<(), Box<dyn Error>> {
        let git_link = self.workdir.join(".git");
        let target_dir_name = format!(".git_{}", remote_name);
        let target_path = self.workdir.join(&target_dir_name);

        let remove_git_link_or_dir = || -> Result<(), std::io::Error> {
            if let Ok(meta) = fs::symlink_metadata(&git_link) {
                if meta.file_type().is_dir() {
                    #[cfg(unix)]
                    fs::remove_file(&git_link)?;
                    #[cfg(windows)]
                    {
                        if meta.is_symlink() {
                            fs::remove_dir(&git_link)?;
                        } else {
                            fs::remove_dir_all(&git_link)?;
                        }
                    }
                } else {
                    fs::remove_file(&git_link)?;
                }
            }
            Ok(())
        };

        remove_git_link_or_dir()?;

        if !target_path.exists() {
            let temp_git = self.workdir.join(".git_temp_init");
            if temp_git.exists() {
                fs::remove_dir_all(&temp_git)?;
            }

            Command::new("git")
                .arg("init")
                .current_dir(&self.workdir)
                .output()?;

            fs::rename(&git_link, &target_path)?;
        } else {
            // TODO: exception
        }

        // 3. Создаем симлинк .git -> .git_origin
        match symlink(&target_path, &git_link) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to link .git to {}: {}", target_dir_name, e).into())
        }
    }
}

impl RepoBackend for GitRepo {
    fn run_cmd(&self, cmd: &str, args: Vec<&str>) -> Result<Output, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
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
        _author: &Author // Пока игнорируем автора для простоты, берем из git config
    ) -> Result<NodeId, Box<dyn Error>> {
        let mut args = vec!["commit-tree", tree_oid, "-m", message];
        for p in parents {
            args.push("-p");
            args.push(&p.0);
        }
        let commit_hash = self.run_git_command(&args)?;
        self.run_git_command(&vec!["update-ref", "HEAD", &commit_hash])?;
        Ok(NodeId(commit_hash))
    }

    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>> {
        self.switch_context(&remote.name)?;

        let refspec = format!("{}:{}", local_tip_id.0, remote_target_ref);
        let args = vec!["push", &remote.url, &refspec];
        self.run_git_command(&args)?;
        Ok(())
    }

    fn is_repo_empty(&self) -> Result<bool, Box<dyn Error>> {
        // Проверяем, есть ли HEAD. Если нет, репозиторий пуст.
        let args = vec!["rev-parse", "--verify", "HEAD"];
        match self.run_git_command(&args) {
            Ok(_) => Ok(false),
            Err(_) => Ok(true),
        }
    }
}