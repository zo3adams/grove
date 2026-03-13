// GROVE — Markdown relationship parser.
// Extracts [[verb -> object]] triples, tags, and metadata from notes.

use regex::Regex;

/// A semantic triple: (subject, verb, object)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Triple {
    pub subject: String,
    pub verb: String,
    pub object: String,
}

/// Parse all `[[verb -> object]]` annotations from markdown text.
/// The subject is derived from the note's filename.
pub fn parse_relationships(subject: &str, markdown: &str) -> Vec<Triple> {
    let re = Regex::new(r"\[\[(.+?)\s*->\s*(.+?)\]\]").expect("Invalid regex");
    re.captures_iter(markdown)
        .map(|cap| Triple {
            subject: subject.to_string(),
            verb: cap[1].trim().to_string(),
            object: cap[2].trim().to_string(),
        })
        .collect()
}

/// Extract the subject name from a file path (filename without .md extension).
pub fn subject_from_path(path: &std::path::Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Extract the first "plain text" sentence from markdown.
/// Skips headings and blank lines, returns the first sentence (up to '.', '!', '?', or end of line).
pub fn first_sentence(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Strip markdown link syntax and relationship annotations
        let clean = trimmed
            .replace(|c: char| c == '*' || c == '_', "");
        let clean = clean.trim().trim_start_matches("- ").trim();
        if clean.is_empty() || clean.starts_with("[[") {
            continue;
        }
        // Find first sentence boundary
        if let Some(pos) = clean.find(|c: char| c == '.' || c == '!' || c == '?') {
            return Some(clean[..=pos].to_string());
        }
        return Some(clean.to_string());
    }
    None
}

/// Parse tags from a `## tags` section in markdown.
/// Returns tags as a list of lowercase trimmed strings, preserving order.
pub fn parse_tags(markdown: &str) -> Vec<String> {
    let mut in_tags_section = false;
    let mut tags = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("## tags") {
            in_tags_section = true;
            continue;
        }
        if in_tags_section {
            if trimmed.starts_with("## ") || trimmed.starts_with("# ") {
                break; // next section
            }
            let tag = trimmed.trim_start_matches("- ").trim();
            if !tag.is_empty() {
                tags.push(tag.to_lowercase());
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let triples = parse_relationships("Mitochondria", "Some text [[produces -> ATP]] more text");
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].subject, "Mitochondria");
        assert_eq!(triples[0].verb, "produces");
        assert_eq!(triples[0].object, "ATP");
    }

    #[test]
    fn test_parse_multiple() {
        let md = "- [[produces -> ATP]]\n- [[located in -> Eukaryotic Cell]]\n[[has membrane -> Inner Membrane]]";
        let triples = parse_relationships("Mitochondria", md);
        assert_eq!(triples.len(), 3);
        assert_eq!(triples[0].object, "ATP");
        assert_eq!(triples[1].verb, "located in");
        assert_eq!(triples[2].object, "Inner Membrane");
    }

    #[test]
    fn test_parse_no_matches() {
        let triples = parse_relationships("Test", "No relationships here.");
        assert!(triples.is_empty());
    }

    #[test]
    fn test_parse_with_spaces() {
        let triples = parse_relationships("A", "[[ has part  ->  Some Thing ]]");
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].verb, "has part");
        assert_eq!(triples[0].object, "Some Thing");
    }

    #[test]
    fn test_subject_from_path() {
        use std::path::Path;
        assert_eq!(subject_from_path(Path::new("vault/Mitochondria.md")), Some("Mitochondria".to_string()));
        assert_eq!(subject_from_path(Path::new("nested/dir/ATP.md")), Some("ATP".to_string()));
    }
}
