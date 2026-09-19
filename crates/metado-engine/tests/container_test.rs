//! Task 1.7: ZIP container reader tests (§10.2)

use std::io::Write;

use metado_engine::container::Container;

#[test]
fn test_read_valid_container() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"name = \"test\"\nversion = \"1.0\"").unwrap();
    zip.start_file("src/main.js", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"export function hello() { return 1; }").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data).unwrap();
    assert!(c.file_exists("mdl.toml"));
    assert!(c.file_exists("src/main.js"));
    let main = c.read_file("src/main.js").unwrap();
    assert!(std::str::from_utf8(&main).unwrap().contains("hello"));
}

#[test]
fn test_missing_mdl_toml_fails() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("src/main.js", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"export function hello() { return 1; }").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data);
    assert!(c.is_err()); // 应报错: 缺少 mdl.toml
}

#[test]
fn test_path_traversal_blocked() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"name = \"test\"\nversion = \"1.0\"").unwrap();
    zip.start_file("../../../etc/passwd", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"evil").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data).unwrap();
    assert!(c.read_file("../../../etc/passwd").is_err()); // 路径穿越被拒
}