use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

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