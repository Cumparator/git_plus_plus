use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use std::process::Command as SysCommand;

/// Обертка над тестовым окружением
struct TestEnv {
    /// Временная директория, удалится сама при выходе из скоупа
    root: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            root: TempDir::new().expect("Failed to create temp dir"),
        }
    }

    /// Путь к рабочей директории теста
    fn path(&self) -> &Path {
        self.root.path()
    }

    /// Запускает gpp с аргументами внутри тестовой папки
    fn gpp(&self) -> Command {
        let mut cmd = Command::cargo_bin("gpp_cli").expect("Binary gpp not found");
        cmd.current_dir(self.path());
        cmd
    }

    /// Запускает обычный git для проверки внутренней кухни
    fn git(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(self.path());
        cmd
    }

    /// Создает файл с контентом
    fn write_file(&self, name: &str, content: &str) {
        let p = self.path().join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).expect("Failed to write file");
    }

    /// Проверяет существование файла/папки
    fn assert_exists(&self, path: &str) {
        assert!(self.path().join(path).exists(), "Path '{}' should exist", path);
    }

    /// Проверяет, что файла нет
    fn assert_missing(&self, path: &str) {
        assert!(!self.path().join(path).exists(), "Path '{}' should NOT exist", path);
    }
}

// --- ТЕСТОВЫЕ СЦЕНАРИИ ---

#[test]
fn test_init_creates_structure() {
    let env = TestEnv::new();

    env.gpp()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Готово!"));

    env.assert_exists(".gitpp");
    env.assert_exists(".gitpp/graph.json");
    env.assert_exists(".git");
    env.assert_exists(".git_origin");
}

#[test]
fn test_basic_workflow_add_log() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    env.write_file("test.txt", "Hello Git++");

    env.gpp()
        .args(&["add", "-m", "First commit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Node created"));

    env.gpp()
        .arg("log")
        .assert()
        .success()
        .stdout(predicate::str::contains("First commit"))
        .stdout(predicate::str::contains("User <user@example.com>"));
}

#[test]
fn test_multicontext_switching() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    env.write_file("file1.txt", "v1");
    let out = env.gpp().args(&["add", "-m", "c1"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let node1_id = stdout.split_whitespace().last().unwrap();

    env.gpp()
        .args(&["chrm", "--remote", "work", "--url", "http://fake", "--node", node1_id])
        .assert()
        .success();

    let git_log = env.git().args(&["log", "--oneline"]).output().expect("git log failed");
    assert!(String::from_utf8_lossy(&git_log.stdout).contains("First commit") ||
        String::from_utf8_lossy(&git_log.stdout).contains("c1"));
}

#[test]
fn test_persistence_graph_json() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();
    env.write_file("a.txt", "A");
    env.gpp().args(&["add", "-m", "save_me"]).assert().success();

    let json_path = env.path().join(".gitpp/graph.json");
    let json_content = fs::read_to_string(json_path).unwrap();

    assert!(json_content.contains("save_me"), "Graph JSON must contain the commit message");
}

#[test]
fn test_checkout_restores_files() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    env.write_file("data.txt", "Version 1");
    let out1 = env.gpp().args(&["add", "-m", "v1"]).output().unwrap();
    let id1 = String::from_utf8(out1.stdout).unwrap().replace("Node created: ", "").trim().to_string();

    env.write_file("data.txt", "Version 2");
    let out2 = env.gpp().args(&["add", "-m", "v2"]).output().unwrap();
    let id2 = String::from_utf8(out2.stdout).unwrap().replace("Node created: ", "").trim().to_string();

    let content = fs::read_to_string(env.path().join("data.txt")).unwrap();
    assert_eq!(content, "Version 2");

    env.gpp().args(&["checkout", &id1]).assert().success();

    let content_v1 = fs::read_to_string(env.path().join("data.txt")).unwrap();
    assert_eq!(content_v1, "Version 1");

    let head_content = fs::read_to_string(env.path().join(".gitpp/HEAD")).unwrap();
    assert_eq!(head_content, id1);
}

#[test]
fn test_chrm_add_and_remove_permissions() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    // 1. Создаем ноду (коммит)
    env.write_file("test_file.txt", "content");
    let out = env.gpp().args(&["add", "-m", "init"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();

    // Вытаскиваем ID созданной ноды из вывода "Node created: <HASH>"
    let node_id = stdout
        .lines()
        .find(|l| l.contains("Node created"))
        .unwrap()
        .replace("Node created: ", "")
        .trim()
        .to_string();

    assert!(!node_id.is_empty(), "Could not extract node ID");

    // 2. Добавляем remote "github" к этой ноде
    env.gpp()
        .args(&[
            "chrm",
            "--remote", "github",
            "--url", "git@github.com:user/repo.git",
            "--node", &node_id
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added permission"));

    // 3. ПРОВЕРКА: Читаем graph.json и ищем там наши данные
    // Мы делаем это "в лоб", читая файл, чтобы убедиться, что данные реально сохранились на диск
    let json_path = env.path().join(".gitpp/graph.json");
    let json_content = fs::read_to_string(&json_path).expect("Failed to read graph.json");

    assert!(json_content.contains("github"), "Graph JSON must contain remote name");
    assert!(json_content.contains("git@github.com:user/repo.git"), "Graph JSON must contain remote URL");

    // 4. Удаляем remote "github" у этой ноды
    env.gpp()
        .args(&[
            "chrm",
            "--remote", "github",
            "--remove",
            "--node", &node_id
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed permission"));

    // 5. ПРОВЕРКА: Снова читаем файл, remote должен исчезнуть
    let json_content_after = fs::read_to_string(&json_path).expect("Failed to read graph.json");
    assert!(!json_content_after.contains("git@github.com:user/repo.git"), "Remote URL should be removed");
}

#[test]
fn test_chrm_fails_without_node_id_argument() {
    // Тест на то, что если не передать ID ноды (и нет HEAD), команда упадет или попросит ID
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    // HEAD пока нет, поэтому chrm должен ругаться или требовать --node
    // (В текущей реализации main.rs он пытается взять HEAD, если node не передан)

    env.gpp()
        .args(&["chrm", "--remote", "origin", "--url", "http://test"])
        .assert()
        .failure(); // Ожидаем ошибку, так как HEAD нет, и --node не передан
}

#[test]
fn test_push_to_local_bare_repository() {
    let env = TestEnv::new();

    // 1. Инициализируем GPP (Клиент)
    env.gpp().arg("init").assert().success();

    // 2. Создаем "Фейковый сервер" (Bare Repo)
    // Это просто соседняя папка, которая будет притворяться Гитхабом
    let remote_dir = tempfile::TempDir::new().unwrap();
    let remote_path = remote_dir.path().to_str().unwrap().to_string();

    // Инициализируем там чистый git (без рабочей копии, только база)
    let setup_status = SysCommand::new("git")
        .args(&["init", "--bare"])
        .current_dir(&remote_dir)
        .output()
        .expect("Failed to init bare repo");
    assert!(setup_status.status.success());

    // 3. Создаем работу в Клиенте
    env.write_file("code.rs", "fn main() {}");
    let out = env.gpp().args(&["add", "-m", "feature_x"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();

    // Вытаскиваем ID ноды
    let node_id = stdout
        .lines()
        .find(|l| l.contains("Node created"))
        .unwrap()
        .replace("Node created: ", "")
        .trim()
        .to_string();

    // 4. Разрешаем пуш (chrm)
    // В качестве URL используем абсолютный путь к папке remote_dir
    env.gpp()
        .args(&[
            "chrm",
            "--node", &node_id,
            "--remote", "local_server",
            "--url", &remote_path
        ])
        .assert()
        .success();

    // 5. ТЕСТИРУЕМ ПУШ
    env.gpp()
        .args(&[
            "push",
            "--node", &node_id,
            "--remote", "local_server",
            "--url", &remote_path  // <--- ДОБАВЬТЕ ЭТУ СТРОКУ!
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Успешно обновлена ссылка"));

    // 6. ПРОВЕРКА: Смотрим, что долетело на "Сервер"
    // Используем обычный git, чтобы прочитать лог в папке remote_dir
    let verify_cmd = SysCommand::new("git")
        .arg("--git-dir")
        .arg(remote_dir.path())
        .args(&["log", "--oneline", "main"])
        .output()
        .expect("Failed to read remote log");

    let log_output = String::from_utf8(verify_cmd.stdout).unwrap();

    // Проверяем, что в логе сервера есть наше сообщение коммита
    assert!(log_output.contains("feature_x"), "Remote repo should contain the pushed commit");
}