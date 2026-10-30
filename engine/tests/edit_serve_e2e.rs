#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! E2E: редакторский сервер `--edit-serve` через реальный дочерний процесс.
//! Валидирует контракт, на котором построен GUI-клиент poler-edit-qt.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

struct Server {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_poler-engine"))
            .arg("--edit-serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn poler-engine --edit-serve");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self { child, stdin, stdout }
    }

    fn send(&mut self, v: Value) {
        let line = v.to_string();
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Читает строки до ответа с нужным id (пропуская события progress).
    fn recv(&mut self, id: u64) -> Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if std::time::Instant::now() > deadline {
                panic!("timeout: нет ответа id={id}");
            }
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            assert!(n > 0, "server closed stdout");
            let v: Value = serde_json::from_str(line.trim()).expect("json line");
            if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                return v;
            }
            // {"ev":"progress",...} — пропускаем
        }
    }

    fn rpc(&mut self, id: u64, v: Value) -> Value {
        self.send(v);
        self.recv(id)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"{\"id\":9999,\"cmd\":\"quit\"}\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

#[test]
fn edit_serve_full_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("doc.txt");
    std::fs::write(&file, "first line\nsecond line\nthird\n").unwrap();

    let mut s = Server::spawn();

    // open
    let r = s.rpc(1, json!({"id": 1, "cmd": "open", "path": file}));
    assert_eq!(r["ok"], json!(true), "open: {r}");
    let doc = r["doc"].as_u64().unwrap();
    assert_eq!(r["bytes"], json!(29));

    // viewport
    let r = s.rpc(2, json!({"id": 2, "cmd": "viewport", "doc": doc, "line": 0, "count": 10}));
    assert_eq!(r["ok"], json!(true));
    let lines = r["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0]["text"], json!("first line"));
    assert_eq!(lines[2]["text"], json!("third"));

    // index (полный line-index; событий progress может не быть на мелком файле)
    let r = s.rpc(3, json!({"id": 3, "cmd": "index", "doc": doc}));
    assert_eq!(r["ok"], json!(true));
    assert_eq!(r["lines"], json!(4));

    // insert_at: вставка в начало второй строки
    let r = s.rpc(
        4,
        json!({"id": 4, "cmd": "insert_at", "doc": doc, "line": 1, "col": 0, "text": "NEW "}),
    );
    assert_eq!(r["ok"], json!(true), "insert_at: {r}");

    // viewport отражает правку
    let r = s.rpc(5, json!({"id": 5, "cmd": "viewport", "doc": doc, "line": 1, "count": 1}));
    assert_eq!(r["lines"][0]["text"], json!("NEW second line"));

    // search по правке (кириллица не нужна — базовая семантика)
    let r = s.rpc(
        6,
        json!({"id": 6, "cmd": "search", "doc": doc, "query": "NEW", "case_sensitive": true, "limit": 10}),
    );
    assert_eq!(r["ok"], json!(true));
    assert_eq!(r["hits"].as_array().unwrap().len(), 1);
    assert_eq!(r["hits"][0]["line"], json!(1));
    assert_eq!(r["hits"][0]["col"], json!(0));

    // linecol: байт в начале 3-й строки (после вставки "NEW " строка 2 на 11+16=27)
    let r = s.rpc(7, json!({"id": 7, "cmd": "linecol", "doc": doc, "byte": 27}));
    assert_eq!(r["line"], json!(2));
    let r = s.rpc(8, json!({"id": 8, "cmd": "goto", "doc": doc, "line": 2}));
    assert_eq!(r["byte"], json!(27));

    // save
    let r = s.rpc(9, json!({"id": 9, "cmd": "save", "doc": doc}));
    assert_eq!(r["ok"], json!(true));
    let disk = std::fs::read_to_string(&file).unwrap();
    assert_eq!(disk, "first line\nNEW second line\nthird\n");

    // undo + save — файл вернулся к оригиналу
    let r = s.rpc(10, json!({"id": 10, "cmd": "undo", "doc": doc}));
    assert_eq!(r["applied"], json!(true));
    let r = s.rpc(11, json!({"id": 11, "cmd": "save", "doc": doc}));
    assert_eq!(r["ok"], json!(true));
    let disk2 = std::fs::read_to_string(&file).unwrap();
    assert_eq!(disk2, "first line\nsecond line\nthird\n");

    // stats
    let r = s.rpc(12, json!({"id": 12, "cmd": "stats", "doc": doc}));
    assert_eq!(r["bytes"], json!(29));
    assert_eq!(r["lines"], json!(4));

    // второй документ (мультидокументность — вкладки GUI)
    let file2 = tmp.path().join("two.txt");
    std::fs::write(&file2, "український текст\nрядок два\n").unwrap();
    let r = s.rpc(13, json!({"id": 13, "cmd": "open", "path": file2}));
    let doc2 = r["doc"].as_u64().unwrap();
    assert_ne!(doc, doc2);
    let r = s.rpc(
        14,
        json!({"id": 14, "cmd": "search", "doc": doc2, "query": "рядок", "case_sensitive": false, "limit": 5}),
    );
    assert_eq!(r["hits"].as_array().unwrap().len(), 1);

    // delete: убрать слово "NEW " после повторной вставки
    let r = s.rpc(
        15,
        json!({"id": 15, "cmd": "insert_at", "doc": doc, "line": 1, "col": 0, "text": "XY "}),
    );
    assert_eq!(r["ok"], json!(true));
    let r = s.rpc(
        16,
        json!({"id": 16, "cmd": "delete", "doc": doc,
               "start_line": 1, "start_col": 0, "end_line": 1, "end_col": 3}),
    );
    assert_eq!(r["ok"], json!(true), "delete: {r}");
    let r = s.rpc(17, json!({"id": 17, "cmd": "viewport", "doc": doc, "line": 1, "count": 1}));
    assert_eq!(r["lines"][0]["text"], json!("second line"));

    // неизвестная команда -> ошибка, сервер жив
    let r = s.rpc(18, json!({"id": 18, "cmd": "no_such"}));
    assert_eq!(r["ok"], json!(false));

    // quit
    s.send(json!({"id": 100, "cmd": "quit"}));
    let _ = s.child.wait();
}

#[test]
fn edit_serve_bad_path_reports_error() {
    let mut s = Server::spawn();
    let r = s.rpc(1, json!({"id": 1, "cmd": "open", "path": "/no/such/file/at/all.txt"}));
    assert_eq!(r["ok"], json!(false));
    assert!(r["error"].as_str().unwrap().len() > 0);
}
