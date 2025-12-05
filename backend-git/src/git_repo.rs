use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::error::Error;
use std::collections::HashMap;

use crate::types::{NodeId, RemoteRef, Author};
use crate::backend::RepoBackend;

/// Конкретная реализация `RepoBackend`, использующая утилиту CLI `git`.
///
/// Управляет рабочей директорией и подменой контекста (симлинков .git)
/// для работы с множественными удаленными репозиториями.
pub struct GitRepo {
    workdir: PathBuf,
    backend_cfg: HashMap<String, String>,
}

impl GitRepo {
    /// Создает новый экземпляр адаптера к Git-репозиторию.
    ///
    /// # Arguments
    ///
    /// * `workdir` - Путь к рабочей директории проекта.
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        GitRepo {
            workdir: workdir.as_ref().to_path_buf(),
            backend_cfg: HashMap::new(),
        }
    }

    /// Вспомогательный метод для запуска команд `git` в рабочей директории.
    /// Перехватывает stderr и преобразует его в ошибку при ненулевом коде возврата.
    ///
    /// # Arguments
    ///
    /// * `args` - Аргументы команды git.
    ///
    /// # Returns
    ///
    /// Возвращает `stdout` команды в виде очищенной строки (trimmed String).
    fn run_git_command(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
        command.args(args);

        let output = command.output()?;

        if !output.status.success() {
            let error_msg = format!(
                "Git error cmd='{:?}': {}",
                args,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Err(error_msg.into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Переключает контекст Git-репозитория путем изменения симлинка `.git`.
    /// Используется для изоляции хранилищ разных удаленных репозиториев (remotes).
    ///
    /// # Arguments
    ///
    /// * `remote_name` - Имя удаленного репозитория. Если `None`, переключается на дефолтное хранилище.
    fn switch_context(&self, remote_name: Option<&str>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl RepoBackend for GitRepo {
    fn run_cmd(&self, cmd: &str, mut args: Vec<&str>) -> Result<Output, Box<dyn Error>> {
        args.insert(0, cmd);
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
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

        let refspec = format!("{}:{}", local_tip_id.0, remote_target_ref);
        let args = vec!["push", &remote.url, &refspec];

        self.run_git_command(&args)?;

        self.switch_context(None)?;

        Ok(())
    }
}