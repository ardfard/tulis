//! Markdown export plus a JSON sidecar that keeps the full document model.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Document;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("could not encode sidecar: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Opened {
    pub document: Document,
    pub notices: Vec<String>,
}

#[derive(Serialize)]
struct SidecarOut<'a> {
    format_version: u32,
    md_hash: String,
    document: &'a Document,
}

#[derive(Deserialize)]
struct SidecarHeader {
    format_version: u32,
}

#[derive(Deserialize)]
struct SidecarIn {
    md_hash: String,
    document: Document,
}

pub fn sidecar_path(md: &Path) -> PathBuf {
    let mut name = md.as_os_str().to_owned();
    name.push(".tulis.json");
    PathBuf::from(name)
}

pub fn export_markdown(doc: &Document) -> String {
    let paragraphs = doc.export_paragraphs();
    if paragraphs.is_empty() {
        return String::new();
    }
    let mut md = paragraphs.join("\n\n");
    md.push('\n');
    md
}

/// Split Markdown into paragraphs on blank lines; lines within a paragraph are joined with spaces.
pub fn parse_markdown(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in md.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                out.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        out.push(current.join(" "));
    }
    out
}

fn hash_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let tmp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Write the Markdown export, then the sidecar. Each file is replaced atomically.
pub fn save(doc: &Document, md_path: &Path) -> Result<(), StorageError> {
    let md = export_markdown(doc);
    atomic_write(md_path, md.as_bytes())?;
    let sidecar = SidecarOut {
        format_version: FORMAT_VERSION,
        md_hash: hash_hex(md.as_bytes()),
        document: doc,
    };
    let json = serde_json::to_vec_pretty(&sidecar)?;
    atomic_write(&sidecar_path(md_path), &json)?;
    Ok(())
}

pub fn open(md_path: &Path) -> Result<Opened, StorageError> {
    let bytes = fs::read(md_path)?;
    let texts = parse_markdown(&String::from_utf8_lossy(&bytes));
    let mut notices = Vec::new();
    let document = match read_sidecar(&sidecar_path(md_path), &mut notices) {
        None => Document::from_texts(&texts),
        Some(side) if side.md_hash == hash_hex(&bytes) => side.document,
        Some(side) => {
            let (doc, lost) = reconcile(&texts, side.document);
            let name = md_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            notices.push(format!(
                "{name} changed outside Tulis; {lost} paragraphs lost alternates/ghosts"
            ));
            doc
        }
    };
    Ok(Opened { document, notices })
}

fn read_sidecar(path: &Path, notices: &mut Vec<String>) -> Option<SidecarIn> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            notices.push(format!("Ignoring unreadable sidecar: {e}"));
            return None;
        }
    };
    match serde_json::from_slice::<SidecarHeader>(&bytes) {
        Ok(header) if header.format_version == FORMAT_VERSION => {}
        Ok(header) => {
            notices.push(format!(
                "Ignoring sidecar with unknown format version {}",
                header.format_version
            ));
            return None;
        }
        Err(e) => {
            notices.push(format!("Ignoring unreadable sidecar: {e}"));
            return None;
        }
    }
    match serde_json::from_slice::<SidecarIn>(&bytes) {
        Ok(sidecar) => Some(sidecar),
        Err(e) => {
            notices.push(format!("Ignoring unreadable sidecar: {e}"));
            None
        }
    }
}

/// Rebuild a document from Markdown paragraphs, reusing sidecar paragraphs whose export text
/// matches exactly (each at most once). Overflow is kept. Returns the document and the number of
/// sidecar paragraphs with alternates/ghosts that could not be matched.
pub fn reconcile(texts: &[String], mut sidecar: Document) -> (Document, usize) {
    let old = std::mem::take(&mut sidecar.paragraphs);
    let old_texts: Vec<String> = old.iter().map(|p| p.export_text()).collect();
    let mut used = vec![false; old.len()];
    let mut paragraphs = Vec::with_capacity(texts.len());
    for text in texts {
        match (0..old.len()).find(|&i| !used[i] && old_texts[i] == *text) {
            Some(i) => {
                used[i] = true;
                paragraphs.push(old[i].clone());
            }
            None => {
                let p = sidecar.new_paragraph(text.as_str());
                paragraphs.push(p);
            }
        }
    }
    let lost = old
        .iter()
        .zip(&used)
        .filter(|(p, used)| !**used && p.has_metadata())
        .count();
    if paragraphs.is_empty() {
        let p = sidecar.new_paragraph("");
        paragraphs.push(p);
    }
    sidecar.paragraphs = paragraphs;
    (sidecar, lost)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Utc;

    use super::*;
    use crate::pos::DocPos;

    fn rich_doc() -> Document {
        let mut d = Document::from_texts(&["I love tea.", "Second paragraph.", "Ghost me."]);
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "adore");
        d.toggle_ghost(2, 0..9).unwrap();
        d.to_overflow(DocPos::new(1, 6)..DocPos::new(1, 16), Utc::now())
            .unwrap();
        d
    }

    #[test]
    fn export_skips_ghosts_and_empty_paragraphs() {
        assert_eq!(export_markdown(&rich_doc()), "I adore tea.\n\nSecond.\n");
        assert_eq!(export_markdown(&Document::new()), "");
    }

    #[test]
    fn parse_joins_lines_and_splits_on_blank_lines() {
        assert_eq!(
            parse_markdown("a\nb\n\n\n c \r\n\r\nd"),
            vec!["a b", "c", "d"]
        );
        assert!(parse_markdown("").is_empty());
    }

    #[test]
    fn sidecar_path_appends_suffix() {
        assert_eq!(
            sidecar_path(Path::new("/x/essay.md")),
            PathBuf::from("/x/essay.md.tulis.json")
        );
    }

    #[test]
    fn save_then_open_round_trips_everything() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("essay.md");
        let doc = rich_doc();
        save(&doc, &path).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "I adore tea.\n\nSecond.\n"
        );
        let opened = open(&path).unwrap();
        assert_eq!(opened.document, doc);
        assert!(opened.notices.is_empty());
        let mut names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["essay.md", "essay.md.tulis.json"]);
    }

    #[test]
    fn open_plain_markdown_without_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.md");
        fs::write(&path, "One.\n\nTwo.\n").unwrap();
        let opened = open(&path).unwrap();
        let texts: Vec<_> = opened
            .document
            .paragraphs
            .iter()
            .map(|p| p.text())
            .collect();
        assert_eq!(texts, vec!["One.", "Two."]);
        assert!(opened.notices.is_empty());
    }

    #[test]
    fn drift_keeps_matching_paragraphs_and_reports_losses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("essay.md");
        save(&rich_doc(), &path).unwrap();
        fs::write(&path, "I adore tea.\n\nSomething new.\n").unwrap();
        let opened = open(&path).unwrap();
        let doc = opened.document;
        assert!(doc.paragraphs[0].has_metadata());
        assert_eq!(doc.paragraphs[1].text(), "Something new.");
        assert_eq!(doc.paragraphs.len(), 2);
        assert_eq!(doc.overflow.len(), 1);
        assert_eq!(
            opened.notices,
            vec!["essay.md changed outside Tulis; 1 paragraphs lost alternates/ghosts".to_string()]
        );
    }

    #[test]
    fn corrupt_sidecar_is_ignored_with_notice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "Text.\n").unwrap();
        fs::write(sidecar_path(&path), "{not json").unwrap();
        let opened = open(&path).unwrap();
        assert_eq!(opened.document.paragraphs[0].text(), "Text.");
        assert!(opened.notices[0].starts_with("Ignoring unreadable sidecar"));
    }

    #[test]
    fn unknown_sidecar_version_is_ignored_with_notice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        fs::write(&path, "Text.\n").unwrap();
        fs::write(sidecar_path(&path), r#"{"format_version": 99}"#).unwrap();
        let opened = open(&path).unwrap();
        assert_eq!(
            opened.notices,
            vec!["Ignoring sidecar with unknown format version 99".to_string()]
        );
    }

    #[test]
    fn open_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            open(&dir.path().join("nope.md")),
            Err(StorageError::Io(_))
        ));
    }
}
