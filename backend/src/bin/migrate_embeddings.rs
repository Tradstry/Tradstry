// OBSOLETE: this bin re-embedded points stored in Qdrant. Qdrant has been
// replaced by Postgres + pgvector, so this migration path no longer applies.
//
// The canonical data migration is now "re-run reindex per account" through the
// AI jobs pipeline (JOB_REINDEX_ACCOUNT_SOURCES), which rebuilds the
// `vector_documents` rows from source. This stub is kept only so the workspace
// keeps compiling.

fn main() {
    eprintln!(
        "migrate_embeddings is obsolete: Qdrant has been removed in favour of \
         Postgres + pgvector. Re-index per account via the AI jobs reindex path \
         (JOB_REINDEX_ACCOUNT_SOURCES) instead."
    );
    std::process::exit(1);
}
