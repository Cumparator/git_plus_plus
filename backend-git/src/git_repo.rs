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
use gpp_core::Node;

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

        // такой огород потому что симлинки удаляются на винде и в линуксе по-разному
        if git_link.exists() || fs::symlink_metadata(&git_link).is_ok() {
            if let Err(_) = fs::remove_file(&git_link) {
                if let Err(e) = fs::remove_dir(&git_link) {
                    return Err(format!("Failed to remove existing .git link: {}", e).into());
                }
            }
        }

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
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(Path::new(&target_dir_name), &git_link)?;
        }

        #[cfg(windows)]
        {
            // Windows: Используем Junction Point через mklink /J.
            // Это обходит требование прав администратора (os error 5).
            // Мы вызываем cmd, так как в std нет нативной поддержки junction без сторонних крейтов.
            // короче говоря сраная винда как всегда суёт костыли в колёса
            let status = Command::new("cmd")
                .args(["/C", "mklink", "/J", ".git", &target_dir_name])
                .current_dir(&self.workdir)
                .output()?
                .status;

            if !status.success() {
                return Err(format!(
                    "Failed to create junction for context '{}'. Ensure you are not blocking .git folder.",
                    remote_name
                ).into());
            }
        }

        Ok(())
    }

    fn get_index_lock_path(&self) -> std::path::PathBuf {
        self.workdir.join(".git").join("index.lock")
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
        //self.switch_context(&remote.name)?;

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

    fn checkout_node(&self, node: &Node) -> Result<(), Box<dyn Error>> {
        let target_context = if let Some(remote) = node.remotes.iter().next() {
            &remote.name
        } else {
            "origin"
        };

        println!("DEBUG: Node {} belongs to '{}'. Switching...", node.id.0, target_context);

        self.switch_context(target_context)?;

        let lock_path = self.get_index_lock_path();
        if lock_path.exists() {
            fs::remove_file(&lock_path).ok();
        }

        let args = vec!["read-tree", "-u", "--reset", &node.payload.tree_id];
        self.run_git_command(&args)?;

        Ok(())
    }
}