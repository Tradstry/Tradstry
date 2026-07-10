//! Pure merge of note *metadata*. No I/O, no clock reads — everything it needs is
//! in its arguments. Per-field LWW by (hlc, client_id), which the stamp encoding
//! already orders lexicographically. The body is a Yjs CRDT and never travels
//! through here; it flows only as append-only update blobs in `note_updates`.

#[derive(Debug, Clone, PartialEq)]
pub struct NoteRow {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub document_json: String,
    pub sort_order: i64,
    pub trade_ids: Vec<String>,
    pub hlc_folder_id: String,
    pub hlc_sort_order: String,
    pub hlc_trade_ids: String,
    pub body_hlc: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug)]
pub enum Merged {
    Unchanged,
    Take(Box<NoteRow>),
    Tombstone,
}

fn sorted(ids: &[String]) -> Vec<String> {
    let mut v = ids.to_vec();
    v.sort();
    v
}

pub fn merge_note(local: &NoteRow, server: &NoteRow) -> Merged {
    // Delete wins. An edit never resurrects a tombstone.
    if server.deleted_at.is_some() || local.deleted_at.is_some() {
        return Merged::Tombstone;
    }

    let mut out = server.clone();

    // Per-field LWW. The stamp encoding sorts lexicographically, and the
    // client_id suffix breaks (millis, counter) ties deterministically.
    if local.hlc_folder_id > server.hlc_folder_id {
        out.folder_id = local.folder_id.clone();
        out.hlc_folder_id = local.hlc_folder_id.clone();
    }
    if local.hlc_sort_order > server.hlc_sort_order {
        out.sort_order = local.sort_order;
        out.hlc_sort_order = local.hlc_sort_order.clone();
    }
    if local.hlc_trade_ids > server.hlc_trade_ids {
        out.trade_ids = local.trade_ids.clone();
        out.hlc_trade_ids = local.hlc_trade_ids.clone();
    }

    // The body is a CRDT owned by `note_updates`; metadata sync must never carry
    // the server body (or its derived title) into the local row.
    out.document_json = local.document_json.clone();
    out.title = local.title.clone();
    out.body_hlc = local.body_hlc.clone();

    let unchanged = out.folder_id == local.folder_id
        && out.sort_order == local.sort_order
        && sorted(&out.trade_ids) == sorted(&local.trade_ids);

    if unchanged {
        Merged::Unchanged
    } else {
        Merged::Take(Box::new(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str) -> NoteRow {
        NoteRow {
            id: id.into(),
            folder_id: None,
            title: "Untitled".into(),
            document_json: "A".into(),
            sort_order: 0,
            trade_ids: vec![],
            hlc_folder_id: String::new(),
            hlc_sort_order: String::new(),
            hlc_trade_ids: String::new(),
            body_hlc: String::new(),
            deleted_at: None,
        }
    }

    #[test]
    fn server_tombstone_beats_local_edit() {
        let mut local = note("n1");
        local.document_json = "local edit".into();
        local.body_hlc = "000000000000009:00000:c1".into();

        let mut server = note("n1");
        server.deleted_at = Some("2026-07-09T00:00:00Z".into());

        assert!(
            matches!(merge_note(&local, &server), Merged::Tombstone),
            "delete wins over a concurrent edit; edits never resurrect a tombstone"
        );
    }

    #[test]
    fn per_field_lww_keeps_both_sides() {
        // Local moved the note; server reordered it. Keep both.
        let mut local = note("n1");
        local.folder_id = Some("f1".into());
        local.hlc_folder_id = "000000000000009:00000:c1".into();

        let mut server = note("n1");
        server.sort_order = 7;
        server.hlc_sort_order = "000000000000009:00000:c2".into();

        match merge_note(&local, &server) {
            Merged::Take(m) => {
                assert_eq!(m.folder_id, Some("f1".into()), "local folder move lost");
                assert_eq!(m.sort_order, 7, "server reorder lost");
            }
            other => panic!("expected Take, got {other:?}"),
        }
    }

    #[test]
    fn trade_ids_compare_as_a_set_not_a_list() {
        let mut local = note("n1");
        local.trade_ids = vec!["b".into(), "a".into()];

        let mut server = note("n1");
        server.trade_ids = vec!["a".into(), "b".into()];

        assert!(
            matches!(merge_note(&local, &server), Merged::Unchanged),
            "reordered trade_ids must not register as a change"
        );
    }

    #[test]
    fn hlc_tie_breaks_on_client_id() {
        let mut local = note("n1");
        local.sort_order = 1;
        local.hlc_sort_order = "000000000000005:00000:aaa".into();

        let mut server = note("n1");
        server.sort_order = 2;
        server.hlc_sort_order = "000000000000005:00000:bbb".into();

        match merge_note(&local, &server) {
            Merged::Take(m) => assert_eq!(m.sort_order, 2, "higher client_id wins the tie"),
            other => panic!("expected Take, got {other:?}"),
        }
    }
}
