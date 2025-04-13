use dvc_data::Tree;
use dvc_data::objects::TreeEntry;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

mod utils;

use utils::write_to_temp_file;
#[test]
pub fn test_tree_load() {
    let dir = t!(tempdir());
    t!(fs::create_dir(dir.path().join("test")));
    let test_dir = dir.path().join("test");

    let contents = r#"[{"md5": "e5a81dd70644b5534aae9f7c32055ec3", "relpath": "data/bar"}, {"md5": "eceec35e3f3dd774244de59b1094cc59", "relpath": "data/foo/baz"}]"#;
    write_to_temp_file(&test_dir, "tree", contents);

    let t = Tree::load_from(&test_dir.join("tree")).unwrap();
    assert_eq!(
        t.entries,
        vec![
            TreeEntry {
                relpath: PathBuf::from("data/bar"),
                oid: "e5a81dd70644b5534aae9f7c32055ec3".to_string()
            },
            TreeEntry {
                relpath: PathBuf::from("data/foo/baz"),
                oid: "eceec35e3f3dd774244de59b1094cc59".to_string()
            },
        ]
    );
}
