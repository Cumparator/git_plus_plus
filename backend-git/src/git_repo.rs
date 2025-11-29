use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::error::Error;
use std::collections::HashMap;

// --- ИМПОРТЫ ЗАВИСИМОСТЕЙ ---
use core::types::{NodeId, RemoteRef};
use core::backend::{RepoBackend};

/// Конкретная реализация RepoBackend, использующая утилиту `git`.
pub struct GitRepo {
    workdir: PathBuf,
    backend_cfg: HashMap<String, String>,
}

impl GitRepo {
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        GitRepo {
            workdir: workdir.as_ref().to_path_buf(),
            backend_cfg: HashMap::new(),
        }
    }

    /// Приватный вспомогательный метод для запуска команд `git`.
    fn run_git_command(&self, args: &[&str]) -> Result<Output, Box<dyn Error>> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
        command.args(args);

        let output = command.output()?;

        if !output.status.success() {
            let error_msg = format!(
                "Git команда завершилась ошибкой: {:?}\nStderr: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(error_msg.into());
        }

        Ok(output)
    }

    /// Инициализирует голый (bare) Git-репозиторий в указанном подкаталоге `name`.
    pub fn init_bare(&self, name: &str) -> Result<(), Box<dyn Error>> {
        let repo_path = self.workdir.join(name);

        let args = vec![
            "init",
            "--bare",
            repo_path.to_str().ok_or("Ошибка кодировки пути")?
        ];

        self.run_git_command(&args)?;
        Ok(())
    }
}

// --- Реализация RepoBackend для GitRepo ---

impl RepoBackend for GitRepo {
    fn run_cmd(&self, cmd: &str, mut args: Vec<&str>) -> Result<Output, Box<dyn Error>> {
        args.insert(0, cmd);
        self.run_git_command(&args)
    }

    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>> {
        // TODO: Использовать `git rev-parse --verify <refname>` для получения хэша.
        unimplemented!()
    }

    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>> {
        // TODO: Использовать `git push <remote.name> <local_tip_id>:<remote_target_ref>`
        unimplemented!()
    }
}