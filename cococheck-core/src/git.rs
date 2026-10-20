use git2::{ErrorCode, Oid, Repository, Sort};
use std::fmt;
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

#[derive(Debug)]
pub enum GitError {
    OpenRepository(git2::Error),
    RevisionNotFound {
        name: &'static str,
        revision: String,
        source: git2::Error,
    },
    NoCommits,
    EmptyRange,
    Git(git2::Error),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::OpenRepository(e) => write!(f, "Failed to open repository: {}", e.message()),
            GitError::RevisionNotFound {
                name,
                revision,
                source,
            } => write!(
                f,
                "The '{}' revision '{}' could not be found: {}",
                name,
                revision,
                source.message()
            ),
            GitError::NoCommits => write!(f, "The repository does not contain any commits."),
            GitError::EmptyRange => write!(
                f,
                "The commit range is empty (possibly 'to' is before 'from')."
            ),
            GitError::Git(e) => write!(f, "Failed to read commits: {}", e.message()),
        }
    }
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
        None => match repo.head() {
            Ok(head) => head
                .target()
                .ok_or_else(|| git2::Error::from_str("HEAD does not point to a commit"))?,
            Err(e) if e.code() == ErrorCode::UnbornBranch => return Err(GitError::NoCommits),
            Err(e) => return Err(e.into()),
        },
    };
    let from_oid = match from {
        Some(revision) => resolve_revision(&repo, "from", revision)?,
        None => find_first_commit(&repo)?,
    };

    let commits = commits_between_inclusive(&repo, from_oid, to_oid)?;
    if commits.is_empty() {
        return Err(GitError::EmptyRange);
    }

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
        .map_err(|source| GitError::RevisionNotFound {
            name,
            revision: revision.to_string(),
            source,
        })?;

    // Peel the object to get the commit an annotated tag points to.
    Ok(object.peel_to_commit()?.id())
}

fn find_first_commit(repo: &Repository) -> Result<Oid, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::REVERSE)?;
    revwalk
        .next()
        .transpose()?
        .ok_or_else(|| git2::Error::from_str("No commits found"))
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
            message: commit
                .message()
                .unwrap_or("<no commit message>")
                .to_string(),
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Signature, Time};
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
