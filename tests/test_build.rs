use dvc_data::ignore::get_ignore;
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

    write_to_temp_file(dir.path(), "foo", "foo\n");
    write_to_temp_file(data_dir.as_path(), "bar", "bar\n");
    write_to_temp_file(data_dir.as_path(), "baz", "baz\n");

    let repo = t!(Repo::open(Some(dir.path().to_path_buf())));
    let state = Some(&repo.state);
    let threads = create_pool(None);

    let abspath = t!(fs::canonicalize(dir.path()));
    let ignore = get_ignore(&repo.root, abspath.parent().unwrap());
    let (obj, size) = build(&repo.odb, dir.path(), state, &ignore, threads);
    assert_eq!(size, 15);
    let t = if let Tree(t) = obj {
        t
    } else {
        panic!("Should have returned tree")
    };

    let oid = t.digest().1;
    assert_eq!(oid, "1517595eb0fce612257347e4c201bcb8.dir");
    assert_eq!(
        t.entries,
        vec![
            (
                PathBuf::from("data/bar"),
                "e5a81dd70644b5534aae9f7c32055ec3".to_string()
            ),
            (
                PathBuf::from("data/baz"),
                "eceec35e3f3dd774244de59b1094cc59".to_string()
            ),
            (
                PathBuf::from("foo"),
                "dbb53f3699703c028483658773628452".to_string()
            ),
        ]
    );
}
