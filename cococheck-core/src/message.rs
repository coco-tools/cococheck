/// The text of the scissors line, after the comment string. Git removes this line and everything
/// below it from the commit message, which is used by `git commit --verbose` to show the diff.
const SCISSORS: &str = " ------------------------ >8 ------------------------";

/// How Git cleans up a commit message before it creates the commit, as configured with
/// `commit.cleanup`. Git's `default` mode is not distinguished from `strip`, because it only differs
/// when the message is not edited, which cannot be determined from the message file.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CleanupMode {
    /// Remove the scissors line and everything below it, and all comment lines, and clean up the
    /// whitespace.
    #[default]
    Strip,

    /// Remove the scissors line and everything below it, and clean up the whitespace.
    Scissors,

    /// Only clean up the whitespace.
    Whitespace,

    /// Do not change the message.
    Verbatim,
}

/// The settings for cleaning up commit messages.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageCleanup {
    pub mode: CleanupMode,
    /// The string that starts comment lines.
    pub comment: String,
}

impl Default for MessageCleanup {
    fn default() -> Self {
        MessageCleanup {
            mode: CleanupMode::default(),
            comment: "#".to_string(),
        }
    }
}

impl MessageCleanup {
    /// Clean up a commit message that was written to a file for editing, like Git does before it
    /// creates the commit, depending on the `mode`: Remove the scissors line and everything below
    /// it, and all lines that start with the `comment` string. Then remove trailing whitespace from
    /// every line, collapse consecutive empty lines, and remove empty lines at the start and the
    /// end of the message.
    pub fn clean(&self, message: &str) -> String {
        if self.mode == CleanupMode::Verbatim {
            return message.to_string();
        }

        let cut_at_scissors = matches!(self.mode, CleanupMode::Strip | CleanupMode::Scissors);
        let remove_comments = self.mode == CleanupMode::Strip;
        let scissors = format!("{}{SCISSORS}", self.comment);

        let mut cleaned = String::new();
        let mut pending_empty_line = false;

        for line in message.lines() {
            if cut_at_scissors && line == scissors {
                break;
            }

            if remove_comments && line.starts_with(&self.comment) {
                continue;
            }

            let line = line.trim_end();
            if line.is_empty() {
                pending_empty_line = !cleaned.is_empty();
                continue;
            }

            if pending_empty_line {
                cleaned.push('\n');
                pending_empty_line = false;
            }

            cleaned.push_str(line);
            cleaned.push('\n');
        }

        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn clean_message(message: &str, comment: &str) -> String {
        clean_with_mode(message, CleanupMode::Strip, comment)
    }

    fn clean_with_mode(message: &str, mode: CleanupMode, comment: &str) -> String {
        let cleanup = MessageCleanup {
            mode,
            comment: comment.to_string(),
        };
        cleanup.clean(message)
    }

    #[test]
    fn keeps_a_clean_message() {
        let message = "feat: Add a feature\n\nBody.\n\nRefs: #1\n";
        assert_eq!(clean_message(message, "#"), message);
    }

    #[test]
    fn appends_a_newline_to_the_last_line() {
        assert_eq!(
            clean_message("feat: Add a feature", "#"),
            "feat: Add a feature\n"
        );
    }

    #[test]
    fn removes_comment_lines() {
        let message = "feat: Add a feature\n\
            # Please enter the commit message for your changes.\n\
            \n\
            Body.\n\
            #\n\
            # On branch main\n";
        assert_eq!(
            clean_message(message, "#"),
            "feat: Add a feature\n\nBody.\n"
        );
    }

    #[test]
    fn removes_lines_starting_with_the_configured_comment_string() {
        let message = "feat: Add a feature\n\n# Not a comment.\n; A comment.\n";
        assert_eq!(
            clean_message(message, ";"),
            "feat: Add a feature\n\n# Not a comment.\n"
        );
    }

    #[test]
    fn keeps_indented_comment_strings() {
        let message = "feat: Add a feature\n\n    # Code comment.\n";
        assert_eq!(clean_message(message, "#"), message);
    }

    #[test]
    fn removes_everything_below_the_scissors_line() {
        let message = "feat: Add a feature\n\n\
            Body.\n\
            # ------------------------ >8 ------------------------\n\
            # Do not modify or remove the line above.\n\
            diff --git a/file b/file\n\
            +Added line.\n";
        assert_eq!(
            clean_message(message, "#"),
            "feat: Add a feature\n\nBody.\n"
        );
    }

    #[test]
    fn keeps_the_scissors_line_with_a_different_comment_string() {
        let message = format!("feat: Add a feature\n\n#{SCISSORS}\nBody.\n");
        assert_eq!(clean_message(&message, ";"), message);
    }

    #[test]
    fn removes_surplus_whitespace() {
        let message = "\n\nfeat: Add a feature  \n\n\n\nBody.\t\n  \nMore body.\n\n\n";
        assert_eq!(
            clean_message(message, "#"),
            "feat: Add a feature\n\nBody.\n\nMore body.\n"
        );
    }

    #[test]
    fn collapses_empty_lines_around_removed_comments() {
        let message = "feat: Add a feature\n\n# Comment.\n\nBody.\n";
        assert_eq!(
            clean_message(message, "#"),
            "feat: Add a feature\n\nBody.\n"
        );
    }

    #[test]
    fn returns_an_empty_message_if_there_are_only_comments() {
        assert_eq!(clean_message("# Comment.\n\n# Comment.\n", "#"), "");
    }

    const MESSAGE_WITH_COMMENTS: &str = "\nfeat: Add a feature  \n\n\n\
        # Comment.\n\
        Body.\n\
        # ------------------------ >8 ------------------------\n\
        diff --git a/file b/file\n";

    #[test]
    fn keeps_comments_in_scissors_mode() {
        assert_eq!(
            clean_with_mode(MESSAGE_WITH_COMMENTS, CleanupMode::Scissors, "#"),
            "feat: Add a feature\n\n# Comment.\nBody.\n"
        );
    }

    #[test]
    fn keeps_comments_and_scissors_in_whitespace_mode() {
        assert_eq!(
            clean_with_mode(MESSAGE_WITH_COMMENTS, CleanupMode::Whitespace, "#"),
            "feat: Add a feature\n\n# Comment.\nBody.\n\
            # ------------------------ >8 ------------------------\n\
            diff --git a/file b/file\n"
        );
    }

    #[test]
    fn keeps_the_message_in_verbatim_mode() {
        assert_eq!(
            clean_with_mode(MESSAGE_WITH_COMMENTS, CleanupMode::Verbatim, "#"),
            MESSAGE_WITH_COMMENTS
        );
    }
}
