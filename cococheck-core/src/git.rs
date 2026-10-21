use git2::{ErrorCode, Oid, Repository, Sort};
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
}

#[derive(Debug)]
pub struct CommitRange {
    pub from: String,
    pub to: String,
    pub commits: Vec<CommitInfo>,
}

// The git2 errors are not exposed as sources, but only their messages are included, because the
// `Display` implementation of `git2::Error` appends the error class and code, which are not helpful
// for users.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitError {
    #[error("failed to open repository: {}", .0.message())]
    OpenRepository(git2::Error),

    #[error("the '{name}' revision '{revision}' could not be found: {}", .error.message())]
    RevisionNotFound {
        name: &'static str,
        revision: String,
        error: git2::Error,
    },

    #[error("the '{name}' revision '{revision}' does not point to a commit")]
    NotACommit {
        name: &'static str,
        revision: String,
    },

    #[error("the repository does not contain any commits")]
    NoCommits,

    #[error("the 'from' revision '{from}' is not an ancestor of the 'to' revision '{to}'")]
    NotAnAncestor { from: String, to: String },

    #[error("failed to read commits: {}", .0.message())]
    Git(git2::Error),
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        GitError::Git(e)
    }
}

/// Get the commits from `from` to `to` (both inclusive) of the repository at `repo_path`, ordered
/// from oldest to newest. If `from` is not set, start at the first commit. If `to` is not set, end
/// at `HEAD`.
pub fn get_commits(
    repo_path: &Path,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommitRange, GitError> {
    let repo = Repository::open(repo_path).map_err(GitError::OpenRepository)?;

    let to_oid = match to {
        Some(revision) => resolve_revision(&repo, "to", revision)?,
        None => repo
            .head()
            .map_err(|e| match e.code() {
                ErrorCode::UnbornBranch => GitError::NoCommits,
                _ => e.into(),
            })?
            .peel_to_commit()?
            .id(),
    };
    let from_oid = match from {
        Some(revision) => {
            let from_oid = resolve_revision(&repo, "from", revision)?;
            if from_oid != to_oid && !repo.graph_descendant_of(to_oid, from_oid)? {
                return Err(GitError::NotAnAncestor {
                    from: revision.to_string(),
                    to: to.unwrap_or("HEAD").to_string(),
                });
            }
            from_oid
        }
        None => find_first_commit(&repo)?,
    };

    let commits = commits_between_inclusive(&repo, from_oid, to_oid)?;

    Ok(CommitRange {
        from: from_oid.to_string(),
        to: to_oid.to_string(),
        commits,
    })
}

fn resolve_revision(
    repo: &Repository,
    name: &'static str,
    revision: &str,
) -> Result<Oid, GitError> {
    let object = repo
        .revparse_single(revision)
        .map_err(|error| GitError::RevisionNotFound {
            name,
            revision: revision.to_string(),
            error,
        })?;

    // Peel the object to get the commit an annotated tag points to.
    let commit = object.peel_to_commit().map_err(|_| GitError::NotACommit {
        name,
        revision: revision.to_string(),
    })?;

    Ok(commit.id())
}

fn find_first_commit(repo: &Repository) -> Result<Oid, GitError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::REVERSE)?;
    revwalk.next().transpose()?.ok_or(GitError::NoCommits)
}

fn commits_between_inclusive(
    repo: &Repository,
    from: Oid,
    to: Oid,
) -> Result<Vec<CommitInfo>, git2::Error> {
    let from_commit = repo.find_commit(from)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(to)?;

    for parent_oid in from_commit.parent_ids() {
        revwalk.hide(parent_oid)?;
    }

    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let commit = repo.find_commit(oid?)?;
        commits.push(CommitInfo {
            id: commit.id().to_string(),
            message: String::from_utf8_lossy(commit.message_bytes()).into_owned(),
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{ObjectType, Signature, Time};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        (dir, repo)
    }

    /// Create a commit with an empty tree and update `HEAD` to point to it.
    fn commit(repo: &Repository, message: &str, parents: &[Oid]) -> Oid {
        let signature = Signature::new("Test", "test@example.org", &Time::new(0, 0)).unwrap();
        let tree = repo
            .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let parents: Vec<_> = parents
            .iter()
            .map(|oid| repo.find_commit(*oid).unwrap())
            .collect();
        let parents: Vec<_> = parents.iter().collect();
        let oid = repo
            .commit(None, &signature, &signature, message, &tree, &parents)
            .unwrap();
        repo.reference("refs/heads/main", oid, true, "test")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        oid
    }

    /// Create a linear history with the given commit messages.
    fn linear_history(repo: &Repository, messages: &[&str]) -> Vec<Oid> {
        let mut oids: Vec<Oid> = Vec::new();
        for message in messages {
            let parents: Vec<Oid> = oids.last().copied().into_iter().collect();
            oids.push(commit(repo, message, &parents));
        }
        oids
    }

    fn messages(range: &CommitRange) -> Vec<&str> {
        range.commits.iter().map(|c| c.message.as_str()).collect()
    }

    #[test]
    fn returns_all_commits_by_default() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third"]);

        let range = get_commits(dir.path(), None, None).unwrap();

        assert_eq!(messages(&range), vec!["first", "second", "third"]);
        assert_eq!(range.from, oids[0].to_string());
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn returns_commits_between_from_and_to_inclusive() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);

        let range = get_commits(
            dir.path(),
            Some(&oids[1].to_string()),
            Some(&oids[2].to_string()),
        )
        .unwrap();

        assert_eq!(messages(&range), vec!["second", "third"]);
    }

    #[test]
    fn excludes_merged_branches_if_from_is_a_merge_commit() {
        let (dir, repo) = init_repo();
        let base = commit(&repo, "base", &[]);
        let main = commit(&repo, "main", &[base]);
        let side = commit(&repo, "side", &[base]);
        let merge = commit(&repo, "merge", &[main, side]);
        commit(&repo, "after merge", &[merge]);

        let range = get_commits(dir.path(), Some(&merge.to_string()), None).unwrap();

        assert_eq!(messages(&range), vec!["merge", "after merge"]);
    }

    #[test]
    fn resolves_annotated_tags() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);
        let signature = Signature::new("Test", "test@example.org", &Time::new(0, 0)).unwrap();
        for (name, oid) in [("v1", oids[1]), ("v2", oids[2])] {
            let target = repo.find_object(oid, None).unwrap();
            repo.tag(name, &target, &signature, name, false).unwrap();
        }

        let range = get_commits(dir.path(), Some("v1"), Some("v2")).unwrap();

        assert_eq!(messages(&range), vec!["second", "third"]);
        assert_eq!(range.from, oids[1].to_string());
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn fails_if_from_is_after_to() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second"]);

        let result = get_commits(
            dir.path(),
            Some(&oids[1].to_string()),
            Some(&oids[0].to_string()),
        );

        assert!(matches!(result, Err(GitError::NotAnAncestor { .. })));
    }

    #[test]
    fn fails_if_from_is_on_a_different_branch() {
        let (dir, repo) = init_repo();
        let base = commit(&repo, "base", &[]);
        let branch1 = commit(&repo, "branch 1", &[base]);
        let branch2 = commit(&repo, "branch 2", &[base]);

        let result = get_commits(
            dir.path(),
            Some(&branch1.to_string()),
            Some(&branch2.to_string()),
        );

        assert!(matches!(result, Err(GitError::NotAnAncestor { .. })));
    }

    #[test]
    fn returns_commit_ids() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first"]);

        let range = get_commits(dir.path(), None, None).unwrap();

        assert_eq!(
            range.commits,
            vec![CommitInfo {
                id: oids[0].to_string(),
                message: "first".to_string(),
            }]
        );
    }

    #[test]
    fn replaces_invalid_utf8_in_messages() {
        let (dir, repo) = init_repo();
        let tree = repo.treebuilder(None).unwrap().write().unwrap();
        // A message encoded in ISO-8859-1 instead of UTF-8.
        let mut content = format!(
            "tree {tree}\n\
             author Test <test@example.org> 0 +0000\n\
             committer Test <test@example.org> 0 +0000\n\n"
        )
        .into_bytes();
        content.extend_from_slice(b"feat: Add caf\xe9");
        let oid = repo
            .odb()
            .unwrap()
            .write(ObjectType::Commit, &content)
            .unwrap();
        repo.reference("refs/heads/main", oid, true, "test")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();

        let range = get_commits(dir.path(), None, None).unwrap();

        assert_eq!(messages(&range), vec!["feat: Add caf\u{FFFD}"]);
    }

    #[test]
    fn fails_for_unknown_revision() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first"]);

        let result = get_commits(dir.path(), Some("unknown"), None);

        assert!(matches!(
            result,
            Err(GitError::RevisionNotFound { name: "from", .. })
        ));
    }

    #[test]
    fn fails_for_revision_that_is_not_a_commit() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first"]);

        let result = get_commits(dir.path(), None, Some("HEAD^{tree}"));

        assert!(matches!(
            result,
            Err(GitError::NotACommit { name: "to", .. })
        ));
    }

    #[test]
    fn fails_for_repository_without_commits() {
        let (dir, _repo) = init_repo();

        let result = get_commits(dir.path(), None, None);

        assert!(matches!(result, Err(GitError::NoCommits)));
    }

    #[test]
    fn fails_for_invalid_repository() {
        let dir = TempDir::new().unwrap();

        let result = get_commits(dir.path(), None, None);

        assert!(matches!(result, Err(GitError::OpenRepository(_))));
    }
}
