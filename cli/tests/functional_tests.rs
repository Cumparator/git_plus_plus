use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use std::process::Command as SysCommand;

// --- TestEnv (Helper Infrastructure) ---

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
        // Пытаемся найти бинарник "gpp" или "gpp_cli" (в зависимости от сборки)
        let mut cmd = Command::cargo_bin("gpp")
            .or_else(|_| Command::cargo_bin("gpp_cli"))
            .expect("Binary gpp/gpp_cli not found");
        cmd.current_dir(self.path());
        cmd
    }

    /// Запускает обычный git для проверки внутренней кухни
    fn git(&self) -> SysCommand {
        let mut cmd = SysCommand::new("git");
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

    /// Хелпер: вытаскивает ID ноды из stdout команды add
    fn parse_node_id(&self, stdout: &[u8]) -> String {
        let s = String::from_utf8_lossy(stdout);
        s.lines()
            .find(|l| l.contains("Node created"))
            .expect("Output does not contain 'Node created'")
            .replace("Node created: ", "")
            .trim()
            .to_string()
    }
}

// --- БАЗОВЫЕ ТЕСТЫ (Init, Add, Log, Persistence) ---

#[test]
fn test_init_default() {
    let env = TestEnv::new();
    // gpp init (без аргументов) -> создает origin
    env.gpp().arg("init").assert().success().stdout(predicate::str::contains("Готово!"));

    env.assert_exists(".gitpp");
    env.assert_exists(".gitpp/graph.json");
    env.assert_exists(".git");
    env.assert_exists(".git_origin");
}

#[test]
fn test_init_multiple_remotes() {
    let env = TestEnv::new();
    // NEW: gpp init origin work=git@host...
    env.gpp()
        .args(&["init", "personal", "work=git@example.com:corp/repo.git"])
        .assert()
        .success();

    env.assert_exists(".gitpp");
    env.assert_exists(".git_personal");
    env.assert_exists(".git_work");

    // Проверяем, что work имеет прописанный URL внутри конфига git
    let config_path = env.path().join(".git_work/config");
    let config_content = fs::read_to_string(config_path).unwrap();
    assert!(config_content.contains("git@example.com:corp/repo.git"));
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
fn test_persistence_graph_json() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();
    env.write_file("a.txt", "A");
    env.gpp().args(&["add", "-m", "save_me"]).assert().success();

    let json_path = env.path().join(".gitpp/graph.json");
    let json_content = fs::read_to_string(json_path).unwrap();

    assert!(json_content.contains("save_me"), "Graph JSON must contain the commit message");
}

// --- ТЕСТЫ ЛОГИКИ ADD (Наследование и Валидация) ---

#[test]
fn test_add_inherits_remotes() {
    let env = TestEnv::new();
    // Инициализируем 2 контекста
    env.gpp().args(&["init", "repo1", "repo2"]).assert().success();

    // 1. Первый коммит явно во все контексты
    env.write_file("f.txt", "A");
    env.gpp()
        .args(&["add", "-m", "root", "--remotes", "repo1", "repo2"])
        .assert()
        .success();

    // 2. Второй коммит без флагов. Должен унаследовать [repo1, repo2]
    env.write_file("f.txt", "B");
    env.gpp()
        .args(&["add", "-m", "child"])
        .assert()
        .success();

    // Проверяем JSON
    let json_path = env.path().join(".gitpp/graph.json");
    let content = fs::read_to_string(json_path).unwrap();

    // Грубая проверка: упоминаний repo1 и repo2 должно быть много (минимум по 2 раза)
    assert!(content.matches("repo1").count() >= 2);
    assert!(content.matches("repo2").count() >= 2);
}

#[test]
fn test_add_validation_subset_failure() {
    let env = TestEnv::new();
    env.gpp().args(&["init", "origin", "secret"]).assert().success();

    // 1. Создаем коммит ТОЛЬКО в origin
    env.write_file("data.txt", "public info");
    let out = env.gpp()
        .args(&["add", "-m", "public_commit", "--remotes", "origin"])
        .assert()
        .success()
        .get_output()
        .stdout.clone();

    // (Необязательно парсить ID здесь, так как следующий add упадет до создания ноды, но для порядка)
    let _ = env.parse_node_id(&out);

    // 2. Пытаемся от public_commit создать коммит в secret
    // ЭТО ДОЛЖНО УПАСТЬ: разрыв цепочки истории для secret
    env.write_file("data.txt", "secret info");
    env.gpp()
        .args(&["add", "-m", "hack", "--remotes", "secret"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Validation Error"));
}

// --- ТЕСТЫ CHECKOUT И CONTEXT SWITCHING ---

#[test]
fn test_checkout_restores_files() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    env.write_file("data.txt", "Version 1");
    let out1 = env.gpp().args(&["add", "-m", "v1"]).output().unwrap();
    let id1 = env.parse_node_id(&out1.stdout);

    env.write_file("data.txt", "Version 2");
    let out2 = env.gpp().args(&["add", "-m", "v2"]).output().unwrap();
    let _id2 = env.parse_node_id(&out2.stdout);

    let content = fs::read_to_string(env.path().join("data.txt")).unwrap();
    assert_eq!(content, "Version 2");

    env.gpp().args(&["checkout", &id1]).assert().success();

    let content_v1 = fs::read_to_string(env.path().join("data.txt")).unwrap();
    assert_eq!(content_v1, "Version 1");

    let head_content = fs::read_to_string(env.path().join(".gitpp/HEAD")).unwrap();
    assert_eq!(head_content, id1);
}

#[test]
fn test_multicontext_switching_check_log() {
    // Тест из твоего списка: проверяем, что log доступен после добавления нового remote
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    env.write_file("file1.txt", "v1");
    let out = env.gpp().args(&["add", "-m", "c1"]).output().unwrap();
    let node1_id = env.parse_node_id(&out.stdout);

    env.gpp()
        .args(&["chrm", "--remote", "work", "--url", "http://fake", "--node", &node1_id])
        .assert()
        .success();

    let git_log = env.git().args(&["log", "--oneline"]).output().expect("git log failed");
    let log_str = String::from_utf8_lossy(&git_log.stdout);
    assert!(log_str.contains("First commit") || log_str.contains("c1"));
}

// --- ТЕСТЫ CHRM (PERMISSIONS) ---

#[test]
fn test_chrm_add_and_remove_permissions() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();

    // 1. Создаем ноду
    env.write_file("test_file.txt", "content");
    let out = env.gpp().args(&["add", "-m", "init"]).output().unwrap();
    let node_id = env.parse_node_id(&out.stdout);

    // 2. Добавляем remote "github"
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

    // 3. ПРОВЕРКА JSON
    let json_path = env.path().join(".gitpp/graph.json");
    let json_content = fs::read_to_string(&json_path).expect("Failed to read graph.json");

    assert!(json_content.contains("github"), "Graph JSON must contain remote name");
    assert!(json_content.contains("git@github.com:user/repo.git"), "Graph JSON must contain remote URL");

    // 4. Удаляем remote
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

    // 5. ПРОВЕРКА JSON (удаление)
    let json_content_after = fs::read_to_string(&json_path).expect("Failed to read graph.json");
    assert!(!json_content_after.contains("git@github.com:user/repo.git"), "Remote URL should be removed");
}

#[test]
fn test_chrm_fails_without_node_id_argument() {
    let env = TestEnv::new();
    env.gpp().arg("init").assert().success();
    // HEAD нет, ID не передан -> ошибка
    env.gpp()
        .args(&["chrm", "--remote", "origin", "--url", "http://test"])
        .assert()
        .failure();
}

// --- E2E ТЕСТ ПУША (REAL GIT) ---

#[test]
fn test_push_to_local_bare_repository_with_lazy_init() {
    let env = TestEnv::new();

    // 1. Клиент
    env.gpp().arg("init").assert().success();

    // 2. "Сервер" (Bare Repo)
    let remote_dir = TempDir::new().unwrap();
    let remote_path = remote_dir.path().to_str().unwrap().to_string();

    let setup_status = SysCommand::new("git")
        .args(&["init", "--bare"])
        .current_dir(&remote_dir)
        .output()
        .expect("Failed to init bare repo");
    assert!(setup_status.status.success());

    // 3. Создаем работу
    env.write_file("code.rs", "fn main() {}");
    let out = env.gpp().args(&["add", "-m", "feature_x"]).output().unwrap();
    let node_id = env.parse_node_id(&out.stdout);

    // 4. Разрешаем пуш (chrm).
    // В этот момент папка .git_local_server еще НЕ создается (Lazy Init).
    env.gpp()
        .args(&[
            "chrm",
            "--node", &node_id,
            "--remote", "local_server",
            "--url", &remote_path
        ])
        .assert()
        .success();

    // Проверим, что папки еще нет (ленивость)
    env.assert_missing(".git_local_server");

    // 5. ТЕСТИРУЕМ ПУШ
    // Вот здесь должна сработать магия: gpp поймет, что .git_local_server нет,
    // создаст его (git init), и выполнит push.
    env.gpp()
        .args(&[
            "push",
            "--node", &node_id,
            "--remote", "local_server",
            // url передаем для надежности, хотя он уже есть в графе,
            // но текущая реализация CLI требует его или берет дефолт
            "--url", &remote_path
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Успешно обновлена ссылка"));

    // Проверим, что папка появилась (ленивость сработала)
    env.assert_exists(".git_local_server");

    // 6. ПРОВЕРКА НА СЕРВЕРЕ
    let verify_cmd = SysCommand::new("git")
        .arg("--git-dir")
        .arg(remote_dir.path())
        .args(&["log", "--oneline", "main"])
        .output()
        .expect("Failed to read remote log");

    let log_output = String::from_utf8(verify_cmd.stdout).unwrap();
    assert!(log_output.contains("feature_x"), "Remote repo should contain the pushed commit");
}