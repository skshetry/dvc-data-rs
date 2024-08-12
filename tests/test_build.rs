use dvc_data::ignore::get_ignore;
use dvc_data::objects::TreeEntry;
use dvc_data::repo::Repo;
use dvc_data::Object::Tree;
use dvc_data::{build, create_pool};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

mod utils;

use utils::write_to_temp_file;

#[test]
pub fn test_build() {
    let dir = t!(tempdir());
    t!(fs::create_dir(dir.path().join("data")));
    let data_dir = dir.path().join("data");

    write_to_temp_file(data_dir.as_path(), "bar", "bar\n");
    write_to_temp_file(data_dir.as_path(), "baz", "baz\n");

    let repo = t!(Repo::open(Some(dir.path().to_path_buf())));
    let state = Some(&repo.state);
    let threads = create_pool(None).unwrap();

    let abspath = t!(fs::canonicalize(dir.path()));
    let ignore = get_ignore(&repo.root, abspath.parent().unwrap()).unwrap();
    let (obj, size) = build(&repo.odb, dir.path(), state, &ignore, threads);
    assert_eq!(size, 10);

    let Tree(t) = obj else {
        panic!("Should have returned tree")
    };

    let (text, oid) = t.digest().unwrap();
    assert_eq!(
        text,
        r#"[{"md5": "e5a81dd70644b5534aae9f7c32055ec3", "relpath": "data/bar"}, {"md5": "eceec35e3f3dd774244de59b1094cc59", "relpath": "data/baz"}]"#
    );
    assert_eq!(oid, "a187d325e83704a3fad49b2f2ab67d20.dir");
    assert_eq!(
        t.entries,
        vec![
            TreeEntry {
                relpath: PathBuf::from("data/bar"),
                oid: "e5a81dd70644b5534aae9f7c32055ec3".to_string()
            },
            TreeEntry {
                relpath: PathBuf::from("data/baz"),
                oid: "eceec35e3f3dd774244de59b1094cc59".to_string()
            },
        ]
    );
}
