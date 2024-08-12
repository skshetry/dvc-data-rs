use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};

pub fn get_ignore(repo_root: &PathBuf, upto: &Path) -> Result<Gitignore, ignore::Error> {
    let mut ignore = GitignoreBuilder::new(repo_root);
    for file in upto.ancestors() {
        ignore.add(file.join(".dvcignore"));
        if file == repo_root {
            break;
        }
    }
    ignore.build()
}
