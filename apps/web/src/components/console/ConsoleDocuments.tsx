import { FormEvent, useCallback, useEffect, useState } from "react";

import { deleteDocument, listDocuments, purgeSite } from "../../api";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function ConsoleDocuments() {
  const { token, busy, setBusy, setFlash, refreshAll, documents } = useConsole();

  const [documentQuery, setDocumentQuery] = useState("");
  const [documentSite, setDocumentSite] = useState("");
  const [documentOffset, setDocumentOffset] = useState(0);
  const [purgeSiteInput, setPurgeSiteInput] = useState("");
  const [localDocuments, setLocalDocuments] = useState<Awaited<ReturnType<typeof listDocuments>> | null>(null);

  // Use documents from context as initial data, switch to local when paginating
  const displayDocuments = localDocuments ?? documents;

  const fetchDocuments = useCallback(
    async (offset: number) => {
      try {
        const result = await listDocuments(token, {
          query: documentQuery.trim() || undefined,
          site: documentSite.trim() || undefined,
          offset,
        });
        setLocalDocuments(result);
      } catch (error) {
        setFlash(getErrorMessage(error, "Refresh failed"));
      }
    },
    [token, documentQuery, documentSite, setFlash],
  );

  // Debounced re-fetch when filters change
  useEffect(() => {
    setDocumentOffset(0);
    setLocalDocuments(null);
    const timer = window.setTimeout(() => {
      void fetchDocuments(0);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [fetchDocuments]);

  async function handleDeleteDocument(documentId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await deleteDocument(token, documentId);
      await refreshAll();
      void fetchDocuments(documentOffset);
    } catch (error) {
      setFlash(getErrorMessage(error, "Document delete failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handlePurgeSite(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setFlash(null);
    try {
      const response = await purgeSite(token, purgeSiteInput);
      setFlash(`Deleted ${response.deleted_documents} documents`);
      setDocumentOffset(0);
      await refreshAll();
      void fetchDocuments(0);
    } catch (error) {
      setFlash(getErrorMessage(error, "Site purge failed"));
    } finally {
      setBusy(false);
    }
  }

  function handlePrevious() {
    const newOffset = Math.max(0, documentOffset - 20);
    setDocumentOffset(newOffset);
    void fetchDocuments(newOffset);
  }

  function handleNext() {
    if (displayDocuments?.next_offset != null) {
      const newOffset = displayDocuments.next_offset;
      setDocumentOffset(newOffset);
      void fetchDocuments(newOffset);
    }
  }

  return (
    <section className="panel panel-wide compact-panel">
      <div className="section-header">
        <h2>Indexed documents</h2>
        <span className="section-meta">
          {displayDocuments?.total_estimate ?? 0} total · next {displayDocuments?.next_offset ?? "-"}
        </span>
      </div>
      <div className="inline-form">
        <input
          value={documentQuery}
          onChange={(event) => setDocumentQuery(event.target.value)}
          placeholder="Filter by title or URL"
        />
        <input
          value={documentSite}
          onChange={(event) => setDocumentSite(event.target.value)}
          placeholder="Filter by site"
        />
      </div>
      <form className="inline-form" onSubmit={handlePurgeSite}>
        <input
          value={purgeSiteInput}
          onChange={(event) => setPurgeSiteInput(event.target.value)}
          placeholder="Site to purge"
        />
        <button type="submit" disabled={busy}>
          Purge site
        </button>
      </form>
      <div className="dense-list">
        {displayDocuments?.documents.length ? (
          displayDocuments.documents.map((document) => (
            <div className="compact-row document-row" key={document.id}>
              <div className="row-primary">
                <strong>{document.title}</strong>
                <span>{document.display_url}</span>
              </div>
              <div className="row-meta">
                <span>{document.language}</span>
                <span>{document.content_type}</span>
                <span>{document.word_count} words</span>
                <span>authority {document.site_authority.toFixed(2)}</span>
                <span>{document.last_crawled_at}</span>
              </div>
              <div className="row-meta">
                <span>host {document.host}</span>
                <span>job {document.source_job_id ?? "-"}</span>
                <span>schema v{document.schema_version}</span>
                <span>parser v{document.parser_version}</span>
                <span>index v{document.index_version}</span>
                {document.duplicate_of ? <span>duplicate of {document.duplicate_of}</span> : <span>primary document</span>}
              </div>
              <p>{document.snippet}</p>
              <div className="row-actions topbar-actions">
                <button
                  type="button"
                  className="plain-link"
                  disabled={busy}
                  onClick={() => void handleDeleteDocument(document.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))
        ) : (
          <div className="list-row">No indexed documents match the current filters.</div>
        )}
      </div>
      <div className="inline-form" style={{ marginTop: 12, marginBottom: 0 }}>
        <button
          type="button"
          disabled={documentOffset === 0}
          onClick={handlePrevious}
        >
          Previous
        </button>
        <span className="section-meta">Offset: {documentOffset}</span>
        <button
          type="button"
          disabled={displayDocuments?.next_offset == null}
          onClick={handleNext}
        >
          Next
        </button>
      </div>
    </section>
  );
}
