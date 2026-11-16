//! The rows of the links of a task (`links`, a map by key): read them for tasks, replace the set of a task. A file of ours is
//! three columns (the reference, its checksum as 64 hex digits, its media type); an address is `href`.
//!
//! Pattern: Repository helper (rows to values and back; the checks of a patch are in `patch`).

use std::collections::BTreeMap;
use std::fmt::Write;

use fs_objects::v5;
use fs_objects::v5::codec::{damaged, id_value, int, opt_int, opt_text, reference_of, text};
use fs_tasks_testdata::tasks_v1::{BasicValue, FileRef, Link, MediaType, ObjectId, TaskResult};

use crate::cur::Cur;
use crate::error::adapt;
use crate::stored::marks;
use crate::tables_more::LINK;

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0_u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The links of the given tasks, each task's in the order they were given.
pub async fn of(c: &v5::Conn<'_>, ids: &[ObjectId]) -> TaskResult<BTreeMap<ObjectId, Vec<Link>>> {
    let mut out: BTreeMap<ObjectId, Vec<Link>> = BTreeMap::new();
    for chunk in ids.chunks(400) {
        let sql = format!(
            "SELECT task, key, href, file_iri, file_sha256, file_media, content_type, size, rel, title \
             FROM tasks_link WHERE task IN ({}) ORDER BY task, pos",
            marks(chunk.len())
        );
        let rows = c
            .query(&sql, chunk.iter().map(|i| id_value(*i)).collect())
            .await
            .map_err(adapt)?;
        for row in &rows {
            let mut cur = Cur::new(row);
            let (task, key, href) = (cur.id()?, cur.text()?, cur.opt_text()?);
            let (iri, sha, media) = (cur.opt_text()?, cur.opt_text()?, cur.opt_text()?);
            let file = match (iri, sha, media) {
                (Some(i), Some(s), Some(m)) => Some(FileRef {
                    iri: reference_of(&i).map_err(adapt)?,
                    sha256: unhex(&s).ok_or_else(|| adapt(damaged("link checksum")))?,
                    media_type: MediaType(m),
                }),
                _ => None,
            };
            let content_type = cur.opt_text()?.map(MediaType);
            let size = cur.opt_int()?.map(|n| u64::try_from(n).unwrap_or(0));
            let (rel, title) = (cur.opt_text()?, cur.opt_text()?);
            out.entry(task).or_default().push(Link {
                key,
                href,
                file,
                content_type,
                size,
                rel,
                title,
            });
        }
    }
    Ok(out)
}

/// Replace the links of a task by `links`, in this order (the size of a link fits a signed number: a patch is checked).
pub async fn write(c: &v5::Conn<'_>, task: ObjectId, links: &[Link]) -> TaskResult<()> {
    c.exec(
        "DELETE FROM tasks_link WHERE task = ?",
        vec![id_value(task)],
    )
    .await
    .map_err(adapt)?;
    for (pos, l) in links.iter().enumerate() {
        let file = l.file.as_ref();
        c.insert(
            &LINK,
            vec![
                id_value(task),
                text(&l.key),
                int(i64::try_from(pos).unwrap_or(i64::MAX)),
                opt_text(l.href.as_deref()),
                opt_text(file.map(|f| f.iri.canonical()).as_deref()),
                opt_text(file.map(|f| hex(&f.sha256)).as_deref()),
                opt_text(file.map(|f| f.media_type.0.clone()).as_deref()),
                opt_text(l.content_type.as_ref().map(|m| m.0.as_str())),
                opt_int(l.size.and_then(|n| i64::try_from(n).ok())),
                opt_text(l.rel.as_deref()),
                opt_text(l.title.as_deref()),
            ],
        )
        .await
        .map_err(adapt)?;
    }
    Ok(())
}
