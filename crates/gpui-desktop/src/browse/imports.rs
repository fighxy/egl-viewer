//! The Imports button and panel (D-175). The button shows only while something is imported (with
//! a count) or while a finished import has a report nobody opened (with a dot); View → Imports
//! opens the panel at any time. The panel names each run and its stage in words, with a bar.

use std::collections::HashSet;

use app_store::Locale;
use library_core::{ImportKind, ImportProgress, ImportState};

use crate::i18n::{t, t_n};

/// What the toolbar shows for the imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportsBadge {
    /// Nothing runs and nothing waits to be read: no button.
    Hidden,
    /// Runs in progress: steps done and in all, summed over them.
    Running { done: u32, total: u32 },
    /// Finished with skipped or failed files (or an Eagle report) not yet opened.
    Report,
}

pub fn is_running(progress: &ImportProgress) -> bool {
    !matches!(progress.state, ImportState::Done | ImportState::Cancelled)
}

/// A finished run worth reading: something was skipped, failed or reported.
pub fn has_report(progress: &ImportProgress) -> bool {
    !is_running(progress)
        && (progress.failed > 0 || progress.duplicates > 0 || progress.report.is_some())
}

/// Steps done and in all for one run: files while copying, thumbnails after.
pub fn steps(progress: &ImportProgress) -> (u32, u32) {
    match progress.state {
        ImportState::Previews => (progress.previews_done, progress.previews_total),
        ImportState::Done | ImportState::Cancelled => (1, 1),
        _ => (progress.processed, progress.total),
    }
}

/// The button's state; `seen` holds the runs whose report was shown.
pub fn badge(imports: &[ImportProgress], seen: &HashSet<u64>) -> ImportsBadge {
    let running: Vec<&ImportProgress> = imports.iter().filter(|p| is_running(p)).collect();
    if !running.is_empty() {
        let (done, total) = running.iter().fold((0, 0), |(d, t), p| {
            let (pd, pt) = steps(p);
            (d + pd, t + pt)
        });
        return ImportsBadge::Running { done, total };
    }
    if imports
        .iter()
        .any(|p| has_report(p) && !seen.contains(&p.id))
    {
        ImportsBadge::Report
    } else {
        ImportsBadge::Hidden
    }
}

/// 0…1 for a run's bar.
pub fn fraction(progress: &ImportProgress) -> f32 {
    let (done, total) = steps(progress);
    if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// "Adding files", "Thumbnails", "Import from Eagle".
pub fn kind_label(locale: Locale, kind: ImportKind) -> String {
    t(
        locale,
        match kind {
            ImportKind::Files => "import.kind.files",
            ImportKind::Previews => "import.kind.previews",
            ImportKind::Eagle => "import.kind.eagle",
        },
    )
}

/// The stage in words.
pub fn state_label(locale: Locale, state: ImportState) -> String {
    t(
        locale,
        match state {
            ImportState::Scanning => "import.state.scanning",
            ImportState::Adding => "import.state.adding",
            ImportState::WaitingForDecision => "import.state.waiting",
            ImportState::Previews => "import.state.previews",
            ImportState::Done => "import.state.done",
            ImportState::Cancelled => "import.state.cancelled",
        },
    )
}

/// The counts under a run: added, duplicates skipped, failures; only those that are not zero.
pub fn summary(locale: Locale, progress: &ImportProgress) -> String {
    let (done, total) = steps(progress);
    let mut parts = Vec::new();
    if is_running(progress) && total > 0 {
        parts.push(format!("{done} / {total}"));
    }
    if progress.added > 0 {
        parts.push(t_n(locale, "import.count.added", progress.added));
    }
    if progress.duplicates > 0 {
        parts.push(t_n(locale, "import.count.duplicates", progress.duplicates));
    }
    if progress.failed > 0 {
        parts.push(t_n(locale, "import.count.failed", progress.failed));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: u64, state: ImportState) -> ImportProgress {
        ImportProgress {
            id,
            kind: ImportKind::Files,
            state,
            total: 40,
            processed: 12,
            added: 0,
            duplicates: 0,
            failed: 0,
            previews_total: 10,
            previews_done: 4,
            current: None,
            duplicate: None,
            failures: Vec::new(),
            skipped: Vec::new(),
            report: None,
            started_at: 0,
            finished_at: None,
        }
    }

    #[test]
    fn hidden_without_work_or_reports() {
        let seen = HashSet::new();
        assert_eq!(badge(&[], &seen), ImportsBadge::Hidden);
        let clean = ImportProgress {
            added: 40,
            ..run(1, ImportState::Done)
        };
        assert_eq!(badge(&[clean], &seen), ImportsBadge::Hidden);
    }

    #[test]
    fn running_runs_count_their_steps() {
        let seen = HashSet::new();
        let copying = run(1, ImportState::Adding);
        let thumbs = run(2, ImportState::Previews);
        assert_eq!(
            badge(&[copying, thumbs], &seen),
            ImportsBadge::Running {
                done: 16,
                total: 50
            }
        );
    }

    #[test]
    fn a_report_waits_until_seen() {
        let failed = ImportProgress {
            failed: 2,
            ..run(7, ImportState::Done)
        };
        let mut seen = HashSet::new();
        assert_eq!(
            badge(std::slice::from_ref(&failed), &seen),
            ImportsBadge::Report
        );
        seen.insert(7);
        assert_eq!(badge(&[failed], &seen), ImportsBadge::Hidden);
        let skipped = ImportProgress {
            duplicates: 3,
            ..run(8, ImportState::Cancelled)
        };
        assert!(has_report(&skipped));
    }

    #[test]
    fn words_and_counts() {
        let locale = Locale::En;
        let p = ImportProgress {
            added: 10,
            failed: 1,
            ..run(1, ImportState::Adding)
        };
        assert!((fraction(&p) - 0.3).abs() < 1e-6);
        let text = summary(locale, &p);
        assert!(text.starts_with("12 / 40"), "{text}");
        assert!(text.contains("10") && text.contains('1'), "{text}");
        assert_ne!(
            state_label(locale, ImportState::Previews),
            "import.state.previews"
        );
        assert_ne!(kind_label(locale, ImportKind::Eagle), "import.kind.eagle");
    }
}
