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

    /// Переключение симлинка .git на нужный backend
    /// Если remote_name = None, переключаем на дефолтный (например .git_primary или просто удаляем линк)
    pub fn switch_context(&self, remote_name: &str) -> Result<(), Box<dyn Error>> {
        let git_link = self.workdir.join(".git");
        let target_dir_name = format!(".git_{}", remote_name);
        let target_path = self.workdir.join(&target_dir_name);

        if !target_path.exists() {
            fs::create_dir_all(&target_path)?;
            Command::new("git")
                .args(&["init", "--bare"]) // Или не bare? Для рабочей копии лучше не bare.
                .current_dir(&target_path)
                .output()?;
        }

        // Удаляем старый симлинк
        if git_link.exists() || fs::symlink_metadata(&git_link).is_ok() {
            if git_link.is_dir() && !fs::symlink_metadata(&git_link)?.file_type().is_symlink() {
                return Err("Found a real .git directory, please rename it to .git_origin manually first".into());
            }
            #[cfg(unix)]
            fs::remove_file(&git_link).ok();
            #[cfg(windows)]
            fs::remove_dir(&git_link).ok();
        }

        // Создаем новый симлинк .git -> .git_origin
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

    fn checkout_tree(&self, tree_oid: &str) -> Result<(), Box<dyn Error>> {
        let args = vec!["read-tree", "-u", "--reset", tree_oid];
        let output = Command::new("git")
            .current_dir(&self.workdir)
            .args(&args)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Git checkout failed: {}", err).into());
        }

        Ok(())
    }
}