import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { deleteDocument, listDocuments, purgeSite } from "../../api";
import { DetailDialog, FieldShell, SectionHeader, StatStrip } from "../common/PanelPrimitives";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatTimestamp(value: string | null) {
  return value ? value.replace("T", " ").replace("Z", "").slice(0, 16) : "-";
}

export function ConsoleDocuments() {
  const { token, busy, setBusy, setFlash, refreshAll, documents } = useConsole();
  const { t } = useTranslation();

  const [documentQuery, setDocumentQuery] = useState("");
  const [documentSite, setDocumentSite] = useState("");
  const [documentOffset, setDocumentOffset] = useState(0);
  const [purgeSiteInput, setPurgeSiteInput] = useState("");
  const [localDocuments, setLocalDocuments] = useState<Awaited<ReturnType<typeof listDocuments>> | null>(null);
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);

  const displayDocuments = localDocuments ?? documents;
  const visibleDocuments = displayDocuments?.documents ?? [];
  const duplicateCount = visibleDocuments.filter((document) => document.duplicate_of).length;
  const primaryCount = visibleDocuments.length - duplicateCount;
  const selectedDocument = useMemo(
    () => visibleDocuments.find((document) => document.id === selectedDocumentId) ?? null,
    [selectedDocumentId, visibleDocuments],
  );

  const fetchDocuments = useCallback(
    async (offset: number) => {
      const result = await listDocuments(token, {
        query: documentQuery.trim() || undefined,
        site: documentSite.trim() || undefined,
        offset,
      });
      setLocalDocuments(result);
      setSelectedDocumentId((current) =>
        current && result.documents.some((document) => document.id === current) ? current : null,
      );
    },
    [token, documentQuery, documentSite],
  );

  useEffect(() => {
    setDocumentOffset(0);
    setLocalDocuments(null);
    const timer = window.setTimeout(() => {
      void fetchDocuments(0).catch((error) => {
        setFlash(getErrorMessage(error, t("console.refresh_failed")));
      });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [fetchDocuments, setFlash, t]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void fetchDocuments(documentOffset).catch(() => undefined);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [documentOffset, fetchDocuments]);

  async function handleDeleteDocument(documentId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await deleteDocument(token, documentId);
      await refreshAll();
      await fetchDocuments(documentOffset);
      if (selectedDocumentId === documentId) {
        setSelectedDocumentId(null);
      }
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.documents.delete_failed")));
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
      setFlash(t("console.documents.purge_success", { count: response.deleted_documents }));
      setDocumentOffset(0);
      setSelectedDocumentId(null);
      await refreshAll();
      await fetchDocuments(0);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.documents.purge_failed")));
    } finally {
      setBusy(false);
    }
  }

  function handlePrevious() {
    const newOffset = Math.max(0, documentOffset - 20);
    setDocumentOffset(newOffset);
    void fetchDocuments(newOffset).catch((error) => {
      setFlash(getErrorMessage(error, t("console.refresh_failed")));
    });
  }

  function handleNext() {
    if (displayDocuments?.next_offset != null) {
      const newOffset = displayDocuments.next_offset;
      setDocumentOffset(newOffset);
      void fetchDocuments(newOffset).catch((error) => {
        setFlash(getErrorMessage(error, t("console.refresh_failed")));
      });
    }
  }

  return (
    <section className="panel panel-wide compact-panel document-panel">
      <SectionHeader
        title={t("console.documents.title")}
        meta={t("console.documents.summary", { count: displayDocuments?.total_estimate ?? 0, next: displayDocuments?.next_offset ?? "-" })}
      />
      <StatStrip
        className="document-summary-strip"
        items={[
          { label: t("console.overview.indexed_docs"), value: displayDocuments?.total_estimate ?? 0 },
          { label: t("console.documents.visible"), value: visibleDocuments.length },
          { label: t("console.overview.duplicates"), value: duplicateCount },
          { label: t("console.documents.primary_count"), value: primaryCount },
        ]}
      />
      <div className="inline-form form-fields document-filter-form">
        <FieldShell className="compact-field" label={t("console.documents.search_label")}>
          <input
            value={documentQuery}
            onChange={(event) => setDocumentQuery(event.target.value)}
            placeholder={t("console.documents.query_placeholder")}
          />
        </FieldShell>
        <FieldShell className="compact-field" label={t("console.documents.site_label")}>
          <input
            value={documentSite}
            onChange={(event) => setDocumentSite(event.target.value)}
            placeholder={t("console.documents.site_placeholder")}
          />
        </FieldShell>
      </div>
      <form className="inline-form form-fields document-purge-form" onSubmit={handlePurgeSite}>
        <FieldShell className="compact-field field-group-wide" label={t("console.documents.purge_site")}>
          <input
            value={purgeSiteInput}
            onChange={(event) => setPurgeSiteInput(event.target.value)}
            placeholder={t("console.documents.purge_placeholder")}
          />
        </FieldShell>
        <button type="submit" disabled={busy}>
          {t("console.documents.purge_site")}
        </button>
      </form>
      <div className="dense-list">
        {visibleDocuments.length ? (
          visibleDocuments.map((document) => (
            <div className="compact-row document-card" key={document.id}>
              <div className="document-toolbar">
                <div className="document-title-group">
                  <div className="row-primary">
                    <strong>{document.title}</strong>
                    <span>{document.display_url}</span>
                  </div>
                  <div className="row-meta row-meta-tight">
                    <span>{document.language}</span>
                    <span>{formatTimestamp(document.last_crawled_at)}</span>
                    <span>{t("console.documents.authority_value", { value: document.site_authority.toFixed(2) })}</span>
                    <span>
                      {document.duplicate_of
                        ? t("console.documents.duplicate_of", { id: document.duplicate_of })
                        : t("console.documents.primary_document")}
                    </span>
                  </div>
                </div>
                <button type="button" className="plain-link" onClick={() => setSelectedDocumentId(document.id)}>
                  {t("console.actions.details")}
                </button>
              </div>
              <p className="document-snippet">{document.snippet}</p>
            </div>
          ))
        ) : (
          <div className="list-row">{t("console.documents.no_documents")}</div>
        )}
      </div>
      <div className="inline-form document-pagination">
        <button
          type="button"
          disabled={documentOffset === 0}
          onClick={handlePrevious}
        >
          {t("search.previous")}
        </button>
        <span className="section-meta">{t("console.documents.offset", { offset: documentOffset })}</span>
        <button
          type="button"
          disabled={displayDocuments?.next_offset == null}
          onClick={handleNext}
        >
          {t("search.next")}
        </button>
      </div>

      <DetailDialog
        open={Boolean(selectedDocument)}
        title={selectedDocument?.title ?? t("console.documents.title")}
        meta={selectedDocument?.display_url}
        closeLabel={t("console.actions.close")}
        onClose={() => setSelectedDocumentId(null)}
        actions={
          selectedDocument ? (
            <button
              type="button"
              className="plain-link"
              disabled={busy}
              onClick={() => void handleDeleteDocument(selectedDocument.id)}
            >
              {t("console.documents.delete")}
            </button>
          ) : null
        }
      >
        {selectedDocument ? (
          <div className="detail-stack">
            <div className="detail-block">
              <span className="field-label">{t("console.documents.host_label")}</span>
              <code>{selectedDocument.canonical_url}</code>
            </div>
            <div className="metadata-grid compact-metadata-wide detail-grid">
              <div>
                <span>{t("console.documents.language")}</span>
                <strong>{selectedDocument.language}</strong>
              </div>
              <div>
                <span>{t("console.documents.content_type_label")}</span>
                <strong>{selectedDocument.content_type}</strong>
              </div>
              <div>
                <span>{t("console.documents.word_count")}</span>
                <strong>{selectedDocument.word_count}</strong>
              </div>
              <div>
                <span>{t("console.documents.authority")}</span>
                <strong>{selectedDocument.site_authority.toFixed(2)}</strong>
              </div>
              <div>
                <span>{t("console.documents.last_crawled")}</span>
                <strong>{formatTimestamp(selectedDocument.last_crawled_at)}</strong>
              </div>
              <div>
                <span>{t("console.documents.host_label")}</span>
                <strong>{selectedDocument.host}</strong>
              </div>
              <div>
                <span>{t("console.documents.job_label")}</span>
                <strong>{selectedDocument.source_job_id ?? "-"}</strong>
              </div>
              <div>
                <span>{t("console.documents.versions_label")}</span>
                <strong>{`s${selectedDocument.schema_version} · p${selectedDocument.parser_version} · i${selectedDocument.index_version}`}</strong>
              </div>
            </div>
            <div className="detail-block">
              <span className="field-label">{t("console.documents.summary_label")}</span>
              <p className="detail-paragraph">{selectedDocument.snippet}</p>
            </div>
          </div>
        ) : null}
      </DetailDialog>
    </section>
  );
}
