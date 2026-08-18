//! Turning canonical paths into paths a person can read.
//!
//! Canonical paths are correct and unreadable. On Windows they carry the
//! extended-length prefix `\\?\`, which exists so the OS skips path parsing and
//! allows >260 characters — it is a kernel detail, and no other editor shows it.
//! Rendering it verbatim in the breadcrumb bar and status bar made every path
//! in the product look corrupted.
//!
//! Display is the only thing these helpers are for. Nothing here may be fed
//! back to the filesystem or to app authority: the canonical form is what those
//! must keep receiving, because stripping a prefix that the OS uses to decide
//! how to parse a path is precisely the kind of "helpful" normalization that
//! turns a long path into a failed open.

/// The Windows extended-length prefix.
const VERBATIM_PREFIX: &str = r"\\?\";
/// The Windows extended-length UNC prefix, which stands in for a leading `\\`.
const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";

/// Strip the Windows extended-length prefix for display.
///
/// Returns a borrowed slice in the common case; only the UNC form has to
/// allocate, because `\\?\UNC\server\share` displays as `\\server\share` and
/// the leading `\\` is not present in the input to borrow from.
#[must_use]
pub fn display_path(path: &str) -> std::borrow::Cow<'_, str> {
    if let Some(rest) = path.strip_prefix(VERBATIM_UNC_PREFIX) {
        std::borrow::Cow::Owned(format!(r"\\{rest}"))
    } else if let Some(rest) = path.strip_prefix(VERBATIM_PREFIX) {
        std::borrow::Cow::Borrowed(rest)
    } else {
        std::borrow::Cow::Borrowed(path)
    }
}

/// The trailing path segments, for a breadcrumb trail.
///
/// A breadcrumb exists to answer "where am I", which the last few segments
/// answer better than an absolute path does — the leading directories are the
/// same for every file in the workspace and so carry no information while
/// consuming the whole bar.
///
/// Both separators are treated as separators regardless of platform: canonical
/// paths in this workspace are produced on whichever OS is running, and a
/// breadcrumb that silently degrades to one long segment is worse than one that
/// splits a literal backslash in a Unix filename, which is vanishingly rare and
/// merely cosmetic here.
#[must_use]
pub fn breadcrumb_trail(path: &str, max_segments: usize) -> Vec<String> {
    if max_segments == 0 {
        return Vec::new();
    }
    let display = display_path(path);
    let segments: Vec<&str> = display
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect();
    let start = segments.len().saturating_sub(max_segments);
    segments[start..]
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extended_length_prefix_is_not_shown_to_anyone() {
        assert_eq!(
            display_path(r"\\?\D:\legion-ide\CHANGELOG.md"),
            r"D:\legion-ide\CHANGELOG.md"
        );
    }

    #[test]
    fn a_verbatim_unc_path_displays_as_an_ordinary_network_path() {
        // `\\?\UNC\server\share` and `\\server\share` name the same location;
        // the second is the one people recognise.
        assert_eq!(
            display_path(r"\\?\UNC\server\share\file.rs"),
            r"\\server\share\file.rs"
        );
    }

    #[test]
    fn paths_without_the_prefix_are_returned_untouched() {
        assert_eq!(
            display_path("/home/dev/project/main.rs"),
            "/home/dev/project/main.rs"
        );
        assert_eq!(
            display_path(r"D:\legion-ide\CHANGELOG.md"),
            r"D:\legion-ide\CHANGELOG.md"
        );
        assert_eq!(display_path(""), "");
    }

    #[test]
    fn a_verbatim_unc_prefix_wins_over_the_shorter_prefix_it_starts_with() {
        // `\\?\UNC\...` also starts with `\\?\`. Testing the shorter prefix
        // first would leave a stray `UNC\` in front of every network path.
        let shown = display_path(r"\\?\UNC\host\share\x");
        assert!(!shown.contains("UNC"), "got {shown}");
    }

    #[test]
    fn a_breadcrumb_keeps_the_end_of_the_path_not_the_start() {
        assert_eq!(
            breadcrumb_trail(r"\\?\D:\legion-ide\crates\legion-desktop\src\view.rs", 3),
            vec!["legion-desktop", "src", "view.rs"]
        );
    }

    #[test]
    fn a_short_path_yields_every_segment_it_has() {
        assert_eq!(breadcrumb_trail("/a/b.rs", 5), vec!["a", "b.rs"]);
    }

    #[test]
    fn repeated_separators_do_not_produce_empty_crumbs() {
        assert_eq!(
            breadcrumb_trail(r"D:\\legion-ide\\\x.rs", 4),
            vec!["D:", "legion-ide", "x.rs"]
        );
    }

    #[test]
    fn asking_for_no_segments_yields_none() {
        assert!(breadcrumb_trail("/a/b/c.rs", 0).is_empty());
    }
}
