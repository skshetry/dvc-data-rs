use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

pub fn get_ignore(repo_root: &Path, upto: &Path) -> Result<Gitignore, ignore::Error> {
    let mut ignore = GitignoreBuilder::new(repo_root);
    for file in upto.ancestors() {
        ignore.add(file.join(".dvcignore"));
        if file == repo_root {
            break;
        }
    }
    ignore.build()
}
