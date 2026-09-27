//! Native MiniMax Code (`mcode`) support.
//!
//! MiniMax Code ships a `mcode` executable that launches a Node runtime on
//! `@minimax-ai/code/cli.js`, plus a `mcode-tools` companion binary. Its runtime
//! data lives in one directory — `~/.minimax` by default, or whatever
//! `MINIMAX_DATA_DIR` / `MAVIS_DATA_DIR` selects.
//!
//! Luvus does **not** open MiniMax Code's private SQLite runtime state, so
//! there is no session discovery: `luvus agent resume <id>` needs a session id
//! that was already reported by a pane, and only
//! `luvus pane report --agent mcode --session <id>` can supply one. A known id
//! resumes with the documented `mcode --session <id>` command. The dated
//! `manifest.json` files under `v2/sessions/` carry a trustworthy `sessionId`,
//! but their only workspace association lives in that SQLite index, and the
//! working directory in the transcript is prose — never a source of identity.

use std::path::{Path, PathBuf};

use super::types::{
    AgentDescriptor, AutomationLaunch, AutomationOperations, IdentityDescriptor, SessionOperations,
};

pub(crate) const NAME: &str = "mcode";

pub(super) const DESCRIPTOR: AgentDescriptor = AgentDescriptor {
    id: NAME,
    // `minimax-code` is the product's own spelling of the executable; `mcode`
    // alone is the canonical form and must stay the only agent id.
    aliases: &["minimax-code"],
    launch_command: "mcode",
    // MiniMax Code takes the task briefing as a positional prompt, so a static
    // separator is not required. A briefing that begins with `-` is still
    // ambiguous to its argument parser; ORCH briefings are plain sentences.
    task_prompt_args: &[],
    automation: Some(AutomationOperations {
        // `exec` is MiniMax Code's documented one-shot entrypoint, but its
        // `--permission` switch only offers `smart`, `full`, and `off`, and
        // `smart` still escalates to an approval request that only a TUI or ACP
        // host can answer. None of them is a reviewed read-only or
        // workspace-scoped switch, so those levels stay unsupported instead of
        // being approximated with a policy that can deny or over-permit work.
        read_only: None,
        workspace: None,
        full_access: Some(AutomationLaunch {
            args: &["exec", "--permission", "full"],
        }),
    }),
    identity: IdentityDescriptor {
        // `mcode` and `minimax code` are portmanteau brand strings, not
        // ordinary words, so they stay trustworthy in screen output. Keep the
        // bare package basename `code` out of this list: MiniMax Code is
        // installed at `node_modules/@minimax-ai/code`, and `code` alone would
        // also claim every other CLI published under a `code` package.
        distinct: DISTINCT_IDENTITIES,
        ambiguous: &[],
        // The official installer execs a Node runtime on `cli.js`, so the live
        // process is the interpreter script rather than a named release binary.
        binary_matcher: None,
        // Keep the npm scope: `@minimax-ai/code` is an exact identity, whereas
        // the `code` basename is shared with unrelated packages.
        interpreter_packages: &["@minimax-ai/code"],
        overlap_priority: 0,
    },
    sessions: Some(SessionOperations {
        // MiniMax Code's dated session manifests record the session id but not
        // the workspace, and the only structured workspace index is its private
        // SQLite runtime state. Skip discovery rather than match a directory
        // name or read the workspace out of transcript prose.
        discovery: None,
        resume: |session| format!("mcode --session {session}\r"),
        // Session Center branching and `/btw` are internal to the live TUI and
        // cannot create a sibling session from an external Luvus command.
        fork: None,
    }),
    // MiniMax Code documents a Plugin system but no hook that reports exact
    // session identity or lifecycle events, so there is no reviewed
    // integration surface to install.
    integration: None,
};

/// Brand spellings that are unmistakable in any evidence, including screen
/// output.
///
/// `minimax-code` is what a running agent actually presents as: the launcher
/// execs a native binary under that name, so it is the identity a real pane
/// exposes. The hyphenated form must be listed explicitly because the spaced
/// `minimax code` brand spelling never matches it. `mcode-tools` names the
/// companion CLI, whose dedicated ACP plugin process is reported separately.
pub(crate) const DISTINCT_IDENTITIES: &[&str] =
    &["minimax-code", "mcode", "mcode-tools", "minimax code"];

/// MiniMax Code's runtime data directory, honoring its own overrides. It reads
/// `MINIMAX_DATA_DIR` first and then the legacy `MAVIS_DATA_DIR` spelling, and
/// trims each value the same way its own runtime does.
pub(crate) fn data_dir_override() -> Option<PathBuf> {
    ["MINIMAX_DATA_DIR", "MAVIS_DATA_DIR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn default_data_dir_at(home: &Path) -> PathBuf {
    home.join(".minimax")
}

fn data_dir_at(home: &Path, override_dir: Option<&Path>) -> PathBuf {
    override_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_data_dir_at(home))
}

/// MiniMax Code's runtime data directory, or `None` when the home directory
/// cannot be resolved. This is the root Luvus checks when deciding whether
/// MiniMax Code is installed on this machine.
pub(crate) fn data_dir() -> Option<PathBuf> {
    let home = crate::platform::home_dir()?;
    Some(data_dir_at(&home, data_dir_override().as_deref()))
}

/// The pure default skill location, used where the caller's home is already
/// known and the ambient environment must not be consulted.
pub(crate) fn default_skill_dir_at(home: &Path) -> PathBuf {
    default_data_dir_at(home).join("skills").join("luvus")
}

/// Where the bundled Luvus skill is installed for MiniMax Code. MiniMax Code
/// loads `<data dir>/skills/<name>/SKILL.md`, which is the bundled layout.
pub(crate) fn skill_dir() -> Result<PathBuf, std::io::Error> {
    Ok(data_dir()
        .ok_or_else(|| std::io::Error::other("home directory not found"))?
        .join("skills")
        .join("luvus"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::AutomationAccess;

    #[test]
    fn automation_advertises_only_full_access() {
        let automation = DESCRIPTOR.automation.expect("mcode runs one-shot work");
        assert!(!automation.supports(AutomationAccess::ReadOnly));
        assert!(!automation.supports(AutomationAccess::Workspace));
        assert_eq!(
            automation
                .launch(AutomationAccess::FullAccess)
                .map(|launch| launch.args),
            Some(["exec", "--permission", "full"].as_slice())
        );
    }

    #[test]
    fn resume_uses_the_documented_session_flag() {
        let resume = DESCRIPTOR.sessions.expect("mcode sessions resume");
        assert_eq!(
            (resume.resume)("mvs_7c98cb72fb0645a0868815991fa57b05"),
            "mcode --session mvs_7c98cb72fb0645a0868815991fa57b05\r"
        );
        assert!(resume.fork.is_none());
        assert!(resume.discovery.is_none());
    }

    #[test]
    fn resume_treats_a_session_id_as_data() {
        let resume = DESCRIPTOR.sessions.expect("mcode sessions resume");
        // A hostile-looking id must stay one argument: no shell splitting, no
        // reordering, and no truncation at the first metacharacter.
        let hostile = "id; rm -rf /";
        assert_eq!(
            (resume.resume)(hostile),
            format!("mcode --session {hostile}\r")
        );
    }

    #[test]
    fn identity_claims_the_scoped_package_but_not_its_bare_basename() {
        assert_eq!(
            DESCRIPTOR.identity.interpreter_packages,
            ["@minimax-ai/code"]
        );
        // A running agent presents as the native `minimax-code` binary, so that
        // hyphenated spelling has to be trustworthy on its own.
        assert!(DESCRIPTOR.identity.distinct.contains(&"minimax-code"));
        assert!(DESCRIPTOR.identity.distinct.contains(&"mcode"));
        // `code` is the shared basename of many unrelated npm packages.
        assert!(!DESCRIPTOR.identity.distinct.contains(&"code"));
        assert!(DESCRIPTOR.identity.ambiguous.is_empty());
        assert!(DESCRIPTOR.identity.binary_matcher.is_none());
    }

    #[test]
    fn skill_dir_follows_the_runtime_data_directory() {
        let home = PathBuf::from("/home/tester");
        assert_eq!(
            default_skill_dir_at(&home),
            PathBuf::from("/home/tester/.minimax/skills/luvus")
        );
        assert_eq!(
            data_dir_at(&home, Some(Path::new("/custom/mcode"))),
            PathBuf::from("/custom/mcode")
        );
    }
}
