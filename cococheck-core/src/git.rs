use crate::message::{CleanupMode, MessageCleanup};
use git2::{
    Commit, DescribeFormatOptions, DescribeOptions, ErrorClass, ErrorCode, Oid, Repository, Sort,
};
use std::path::Path;
use std::process::Command;

#[derive(Debug, PartialEq)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    /// Whether the commit is a merge commit, which has more than one parent.
    pub is_merge: bool,
    /// The author of the commit as `Name <email>`.
    pub author: String,
}

#[derive(Debug)]
pub struct CommitRange {
    /// The resolved `from` commit, or `None` if the range starts at the root commits or contains a
    /// single selected commit.
    pub from: Option<String>,
    /// The resolved `to` commit, or the single selected commit.
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

    #[error(
        "the {} '{revision}' could not be found: {}",
        describe_revision(name),
        .error.message()
    )]
    RevisionNotFound {
        name: &'static str,
        revision: String,
        error: git2::Error,
    },

    #[error(
        "the {} '{revision}' does not point to a commit",
        describe_revision(name)
    )]
    NotACommit {
        name: &'static str,
        revision: String,
    },

    #[error("the repository does not contain any commits")]
    NoCommits,

    #[error("the symmetric difference '{0}' is not supported, use a range like 'A..B' instead")]
    SymmetricDifference(String),

    #[error("failed to read commits: {}", .0.message())]
    Git(git2::Error),

    #[error("failed to determine the author with 'git var GIT_AUTHOR_IDENT': {0}")]
    AuthorIdent(String),
}

/// Describe the revision with the given `name` in error messages: `revision` for a single revision,
/// and e.g. `'from' revision` for one end of a range.
fn describe_revision(name: &str) -> String {
    match name {
        "revision" => name.to_string(),
        _ => format!("'{name}' revision"),
    }
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        GitError::Git(e)
    }
}

/// Get the commits of the repository at `repo_path` that are reachable from `to`, but not from
/// `from`, ordered from oldest to newest. Like Git's `from..to` notation, this excludes `from`
/// itself, and `from` does not have to be an ancestor of `to`. If `from` is not set, include all
/// commits up to the root commits. If `to` is not set, end at `HEAD`.
pub fn get_commits(
    repo_path: &Path,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommitRange, GitError> {
    let repo = Repository::open(repo_path).map_err(GitError::OpenRepository)?;

    let to_oid = resolve_to(&repo, to)?;
    let from_oid = from
        .map(|revision| resolve_revision(&repo, "from", revision))
        .transpose()?;

    let commits = commits_between(&repo, from_oid, to_oid)?;

    Ok(CommitRange {
        from: from_oid.map(|oid| oid.to_string()),
        to: to_oid.to_string(),
        commits,
    })
}

/// Find the latest tag of the repository at `repo_path` that is reachable from `to`, or from `HEAD`
/// if `to` is not set, and return its name. Like `git describe --tags --abbrev=0`, this is the tag
/// with the fewest commits between it and `to`, and both annotated and lightweight tags are
/// considered. Return `None` if no tag is reachable from `to`.
pub fn find_latest_tag(repo_path: &Path, to: Option<&str>) -> Result<Option<String>, GitError> {
    let repo = Repository::open(repo_path).map_err(GitError::OpenRepository)?;
    let to_oid = resolve_to(&repo, to)?;

    let commit = repo.find_commit(to_oid)?;
    let describe = match commit
        .as_object()
        .describe(DescribeOptions::new().describe_tags())
    {
        Ok(describe) => describe,
        // libgit2 reports that no tag is reachable with a describe error, whose code is generic if
        // the repository has no tags at all.
        Err(e) if e.class() == ErrorClass::Describe => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let name = describe.format(Some(DescribeFormatOptions::new().abbreviated_size(0)))?;
    Ok(Some(name))
}

/// Get the commits of the repository at `repo_path` that are selected by `revisions`, ordered from
/// oldest to newest. Like `git show`, a single revision selects only that commit, and a range
/// `from..to` selects the same commits as [`get_commits`]. An omitted end of a range means `HEAD`.
/// The symmetric difference `a...b` is not supported.
pub fn get_commits_for_revisions(
    repo_path: &Path,
    revisions: &str,
) -> Result<CommitRange, GitError> {
    if revisions.contains("...") {
        return Err(GitError::SymmetricDifference(revisions.to_string()));
    }

    if let Some((from, to)) = revisions.split_once("..") {
        let from = if from.is_empty() { "HEAD" } else { from };
        let to = (!to.is_empty()).then_some(to);
        return get_commits(repo_path, Some(from), to);
    }

    let repo = Repository::open(repo_path).map_err(GitError::OpenRepository)?;
    let oid = resolve_revision(&repo, "revision", revisions)?;

    Ok(CommitRange {
        from: None,
        to: oid.to_string(),
        commits: vec![commit_info(&repo.find_commit(oid)?)],
    })
}

/// Return the author that Git records for a new commit in the repository at `repo_path` as
/// `Name <email>`. Like Git, respect the `GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL` environment
/// variables, and the `author.*` and `user.*` settings, by running `git var GIT_AUTHOR_IDENT`, as
/// libgit2 does not implement all of Git's rules for determining the identity.
pub fn author_ident(repo_path: &Path) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["var", "GIT_AUTHOR_IDENT"])
        .output()
        .map_err(|e| GitError::AuthorIdent(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError::AuthorIdent(stderr.trim().to_string()));
    }

    parse_ident(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| GitError::AuthorIdent("unexpected output".to_string()))
}

/// Return the `Name <email>` part of a Git identity like `Name <email> 1700000000 +0100`.
fn parse_ident(ident: &str) -> Option<String> {
    let mut parts = ident.trim_end().rsplitn(3, ' ');
    let (_offset, _timestamp) = (parts.next()?, parts.next()?);
    parts.next().map(str::to_string)
}

/// Return how Git cleans up commit messages edited in the repository at `repo_path`, as configured
/// by `commit.cleanup`, and `core.commentString` or `core.commentChar`. If the repository cannot be
/// opened, use the global Git configuration. Use the defaults for settings that are not set or
/// invalid, or if the configuration cannot be read. The comment string `auto` is also replaced by
/// the default `#`, because the comment character that Git chooses in that case cannot be
/// determined afterwards.
pub fn message_cleanup(repo_path: &Path) -> MessageCleanup {
    let config = match Repository::open(repo_path) {
        Ok(repo) => repo.config(),
        Err(_) => git2::Config::open_default(),
    };

    match config {
        Ok(config) => configured_message_cleanup(&config),
        Err(_) => MessageCleanup::default(),
    }
}

fn configured_message_cleanup(config: &git2::Config) -> MessageCleanup {
    let default = MessageCleanup::default();

    let mode = match config.get_string("commit.cleanup").as_deref() {
        Ok("scissors") => CleanupMode::Scissors,
        Ok("whitespace") => CleanupMode::Whitespace,
        Ok("verbatim") => CleanupMode::Verbatim,
        _ => default.mode,
    };

    let comment = ["core.commentString", "core.commentChar"]
        .iter()
        .find_map(|name| config.get_string(name).ok())
        .filter(|comment| !comment.is_empty() && comment != "auto")
        .unwrap_or(default.comment);

    MessageCleanup { mode, comment }
}

/// Resolve the `to` revision to a commit, or `HEAD` if it is not set.
fn resolve_to(repo: &Repository, to: Option<&str>) -> Result<Oid, GitError> {
    match to {
        Some(revision) => resolve_revision(repo, "to", revision),
        None => Ok(repo
            .head()
            .map_err(|e| match e.code() {
                ErrorCode::UnbornBranch => GitError::NoCommits,
                _ => e.into(),
            })?
            .peel_to_commit()?
            .id()),
    }
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

fn commits_between(
    repo: &Repository,
    from: Option<Oid>,
    to: Oid,
) -> Result<Vec<CommitInfo>, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(to)?;

    if let Some(from) = from {
        revwalk.hide(from)?;
    }

    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

    let mut commits = Vec::new();
    for oid in revwalk {
        commits.push(commit_info(&repo.find_commit(oid?)?));
    }

    Ok(commits)
}

fn commit_info(commit: &Commit) -> CommitInfo {
    CommitInfo {
        id: commit.id().to_string(),
        message: String::from_utf8_lossy(commit.message_bytes()).into_owned(),
        is_merge: commit.parent_count() > 1,
        author: format!(
            "{} <{}>",
            String::from_utf8_lossy(commit.author().name_bytes()),
            String::from_utf8_lossy(commit.author().email_bytes())
        ),
    }
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

    fn cleanup_from(config: &str) -> MessageCleanup {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, config).unwrap();
        configured_message_cleanup(&git2::Config::open(&path).unwrap())
    }

    #[test]
    fn uses_the_default_cleanup_if_nothing_is_configured() {
        assert_eq!(cleanup_from(""), MessageCleanup::default());
    }

    #[test]
    fn reads_the_comment_char() {
        let cleanup = cleanup_from("[core]\n\tcommentChar = \";\"\n");
        assert_eq!(cleanup.comment, ";");
    }

    #[test]
    fn prefers_the_comment_string_over_the_comment_char() {
        let cleanup = cleanup_from("[core]\n\tcommentChar = \";\"\n\tcommentString = //\n");
        assert_eq!(cleanup.comment, "//");
    }

    #[test]
    fn ignores_an_automatic_comment_char() {
        let cleanup = cleanup_from("[core]\n\tcommentChar = auto\n");
        assert_eq!(cleanup.comment, "#");
    }

    #[test]
    fn reads_the_cleanup_mode() {
        let mode = |value: &str| cleanup_from(&format!("[commit]\n\tcleanup = {value}\n")).mode;
        assert_eq!(mode("strip"), CleanupMode::Strip);
        assert_eq!(mode("default"), CleanupMode::Strip);
        assert_eq!(mode("scissors"), CleanupMode::Scissors);
        assert_eq!(mode("whitespace"), CleanupMode::Whitespace);
        assert_eq!(mode("verbatim"), CleanupMode::Verbatim);
        assert_eq!(mode("invalid"), CleanupMode::Strip);
    }

    #[test]
    fn reads_the_cleanup_from_the_repository() {
        let (dir, repo) = init_repo();
        let mut config = repo.config().unwrap();
        config.set_str("core.commentChar", "%").unwrap();
        config.set_str("commit.cleanup", "scissors").unwrap();
        assert_eq!(
            message_cleanup(dir.path()),
            MessageCleanup {
                mode: CleanupMode::Scissors,
                comment: "%".to_string(),
            }
        );
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
        assert_eq!(range.from, None);
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn returns_commits_after_from_up_to_and_including_to() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);

        let range = get_commits(
            dir.path(),
            Some(&oids[0].to_string()),
            Some(&oids[2].to_string()),
        )
        .unwrap();

        assert_eq!(messages(&range), vec!["second", "third"]);
        assert_eq!(range.from, Some(oids[0].to_string()));
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn returns_no_commits_if_from_equals_to() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first", "second"]);

        let range = get_commits(dir.path(), Some("HEAD"), None).unwrap();

        assert_eq!(messages(&range), Vec::<&str>::new());
    }

    #[test]
    fn excludes_commits_merged_from_the_from_branch() {
        let (dir, repo) = init_repo();
        let base = commit(&repo, "base", &[]);
        let main = commit(&repo, "main", &[base]);
        let feature = commit(&repo, "feature", &[base]);
        let merge = commit(&repo, "merge main", &[feature, main]);
        commit(&repo, "after merge", &[merge]);

        let range = get_commits(dir.path(), Some(&main.to_string()), None).unwrap();

        assert_eq!(
            messages(&range),
            vec!["feature", "merge main", "after merge"]
        );
    }

    #[test]
    fn includes_all_parents_of_a_merge_commit_without_from() {
        let (dir, repo) = init_repo();
        let base = commit(&repo, "base", &[]);
        let main = commit(&repo, "main", &[base]);
        let side = commit(&repo, "side", &[base]);
        commit(&repo, "merge", &[main, side]);

        let range = get_commits(dir.path(), None, None).unwrap();

        let mut messages = messages(&range);
        messages.sort();
        assert_eq!(messages, vec!["base", "main", "merge", "side"]);
    }

    /// Create an annotated tag with the given `name` that points to the commit `oid`.
    fn annotated_tag(repo: &Repository, name: &str, oid: Oid) {
        let signature = Signature::new("Test", "test@example.org", &Time::new(0, 0)).unwrap();
        let target = repo.find_object(oid, None).unwrap();
        repo.tag(name, &target, &signature, name, false).unwrap();
    }

    /// Create a lightweight tag with the given `name` that points to the commit `oid`.
    fn lightweight_tag(repo: &Repository, name: &str, oid: Oid) {
        let target = repo.find_object(oid, None).unwrap();
        repo.tag_lightweight(name, &target, false).unwrap();
    }

    #[test]
    fn finds_the_latest_tag_reachable_from_head() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);
        annotated_tag(&repo, "v1", oids[0]);
        annotated_tag(&repo, "v2", oids[1]);

        assert_eq!(
            find_latest_tag(dir.path(), None).unwrap(),
            Some("v2".to_string())
        );
    }

    #[test]
    fn finds_lightweight_tags() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third"]);
        annotated_tag(&repo, "v1", oids[0]);
        lightweight_tag(&repo, "v2", oids[1]);

        assert_eq!(
            find_latest_tag(dir.path(), None).unwrap(),
            Some("v2".to_string())
        );
    }

    #[test]
    fn finds_the_latest_tag_reachable_from_to() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);
        annotated_tag(&repo, "v1", oids[0]);
        annotated_tag(&repo, "v2", oids[2]);

        assert_eq!(
            find_latest_tag(dir.path(), Some(&oids[1].to_string())).unwrap(),
            Some("v1".to_string())
        );
    }

    #[test]
    fn finds_a_tag_on_to_itself() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second"]);
        annotated_tag(&repo, "v1", oids[1]);

        assert_eq!(
            find_latest_tag(dir.path(), None).unwrap(),
            Some("v1".to_string())
        );
    }

    #[test]
    fn finds_no_tag_if_none_is_reachable() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second"]);

        assert_eq!(find_latest_tag(dir.path(), None).unwrap(), None);

        // A tag on a commit that is not reachable from `to` is not found either.
        let side = commit(&repo, "side", &[oids[0]]);
        annotated_tag(&repo, "v1", side);
        assert_eq!(
            find_latest_tag(dir.path(), Some(&oids[1].to_string())).unwrap(),
            None
        );
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

        assert_eq!(messages(&range), vec!["third"]);
        assert_eq!(range.from, Some(oids[1].to_string()));
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn returns_no_commits_if_from_is_after_to() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second"]);

        let range = get_commits(
            dir.path(),
            Some(&oids[1].to_string()),
            Some(&oids[0].to_string()),
        )
        .unwrap();

        assert_eq!(messages(&range), Vec::<&str>::new());
    }

    #[test]
    fn returns_commits_not_on_from_if_from_is_on_a_different_branch() {
        let (dir, repo) = init_repo();
        let base = commit(&repo, "base", &[]);
        let main = commit(&repo, "main", &[base]);
        let feature = commit(&repo, "feature", &[base]);

        let range = get_commits(
            dir.path(),
            Some(&main.to_string()),
            Some(&feature.to_string()),
        )
        .unwrap();

        assert_eq!(messages(&range), vec!["feature"]);
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
                is_merge: false,
                author: "Test <test@example.org>".to_string(),
            }]
        );
    }

    #[test]
    fn parses_the_name_and_email_of_an_ident() {
        assert_eq!(
            parse_ident("Jane Doe <jane@example.org> 1700000000 +0100\n"),
            Some("Jane Doe <jane@example.org>".to_string())
        );
        assert_eq!(parse_ident("invalid"), None);
    }

    #[test]
    fn reads_the_author_ident_from_the_repository_config() {
        let (dir, repo) = init_repo();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Jane Doe").unwrap();
        config.set_str("user.email", "jane@example.org").unwrap();
        // The `author.*` settings take precedence over the `user.*` settings.
        config.set_str("author.name", "Author").unwrap();
        config
            .set_str("author.email", "author@example.org")
            .unwrap();

        assert_eq!(
            author_ident(dir.path()).unwrap(),
            "Author <author@example.org>"
        );
    }

    #[test]
    fn detects_merge_commits_by_their_parents() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second"]);
        let side = commit(&repo, "side", &[oids[0]]);
        commit(&repo, "feat: Merge the side branch", &[oids[1], side]);

        let range = get_commits(dir.path(), None, None).unwrap();

        let merges: Vec<(&str, bool)> = range
            .commits
            .iter()
            .map(|c| (c.message.as_str(), c.is_merge))
            .collect();
        assert_eq!(merges.len(), 4);
        assert!(merges.contains(&("feat: Merge the side branch", true)));
        assert!(merges.contains(&("side", false)));
        assert!(merges.contains(&("second", false)));
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
    fn returns_only_a_single_selected_commit() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third"]);

        let range = get_commits_for_revisions(dir.path(), "HEAD~1").unwrap();

        assert_eq!(messages(&range), vec!["second"]);
        assert_eq!(range.from, None);
        assert_eq!(range.to, oids[1].to_string());
    }

    #[test]
    fn returns_the_commits_of_a_selected_range() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third", "fourth"]);

        let range = get_commits_for_revisions(dir.path(), "HEAD~3..HEAD~1").unwrap();

        assert_eq!(messages(&range), vec!["second", "third"]);
        assert_eq!(range.from, Some(oids[0].to_string()));
        assert_eq!(range.to, oids[2].to_string());
    }

    #[test]
    fn uses_head_for_an_omitted_end_of_a_selected_range() {
        let (dir, repo) = init_repo();
        let oids = linear_history(&repo, &["first", "second", "third"]);
        let first = oids[0].to_string();

        let to_head = get_commits_for_revisions(dir.path(), &format!("{first}..")).unwrap();
        let from_head = get_commits_for_revisions(dir.path(), &format!("..{first}")).unwrap();

        assert_eq!(messages(&to_head), vec!["second", "third"]);
        assert_eq!(messages(&from_head), Vec::<&str>::new());
    }

    #[test]
    fn fails_for_a_symmetric_difference() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first", "second"]);

        let result = get_commits_for_revisions(dir.path(), "HEAD~1...HEAD");

        assert!(matches!(result, Err(GitError::SymmetricDifference(_))));
    }

    #[test]
    fn fails_for_unknown_revisions() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first"]);

        let single = get_commits_for_revisions(dir.path(), "unknown");
        let range = get_commits_for_revisions(dir.path(), "HEAD..unknown");

        assert!(matches!(
            single,
            Err(GitError::RevisionNotFound {
                name: "revision",
                ..
            })
        ));
        assert!(matches!(
            range,
            Err(GitError::RevisionNotFound { name: "to", .. })
        ));
    }

    #[test]
    fn describes_revisions_in_error_messages() {
        let (dir, repo) = init_repo();
        linear_history(&repo, &["first"]);

        let single = get_commits_for_revisions(dir.path(), "HEAD^{tree}").unwrap_err();
        let range = get_commits_for_revisions(dir.path(), "HEAD^{tree}..").unwrap_err();

        assert_eq!(
            single.to_string(),
            "the revision 'HEAD^{tree}' does not point to a commit"
        );
        assert_eq!(
            range.to_string(),
            "the 'from' revision 'HEAD^{tree}' does not point to a commit"
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
