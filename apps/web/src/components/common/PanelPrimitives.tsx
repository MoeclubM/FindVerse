import { useEffect, type ReactNode } from "react";

export function SectionHeader(props: {
  title: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
  heading?: "h2" | "h3";
}) {
  const Heading = props.heading ?? "h2";

  return (
    <div className={props.className ? `section-header ${props.className}` : "section-header"}>
      <div className="section-heading-stack">
        <Heading>{props.title}</Heading>
        {props.meta ? <span className="section-meta">{props.meta}</span> : null}
      </div>
      {props.actions ? <div className="row-actions">{props.actions}</div> : null}
    </div>
  );
}

export function StatStrip(props: {
  items: Array<{
    label: ReactNode;
    value: ReactNode;
  }>;
  className?: string;
}) {
  return (
    <div className={props.className ? `summary-strip ${props.className}` : "summary-strip"}>
      {props.items.map((item, index) => (
        <div key={index}>
          <span>{item.label}</span>
          <strong>{item.value}</strong>
        </div>
      ))}
    </div>
  );
}

export function FieldShell(props: {
  label: ReactNode;
  hint?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={props.className ? `field-group ${props.className}` : "field-group"}>
      <span className="field-label">{props.label}</span>
      {props.children}
      {props.hint ? <span className="field-hint">{props.hint}</span> : null}
    </label>
  );
}

export function DetailDialog(props: {
  open: boolean;
  title: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  closeLabel: ReactNode;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    if (!props.open) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        props.onClose();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [props.open, props.onClose]);

  if (!props.open) {
    return null;
  }

  return (
    <div
      className="detail-dialog-overlay"
      role="presentation"
      onClick={props.onClose}
    >
      <div
        className="detail-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={typeof props.title === "string" ? props.title : undefined}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="detail-dialog-header">
          <SectionHeader
            title={props.title}
            meta={props.meta}
            actions={
              <>
                {props.actions}
                <button type="button" className="plain-link detail-dialog-close" onClick={props.onClose}>
                  {props.closeLabel}
                </button>
              </>
            }
          />
        </div>
        <div className="detail-dialog-body">{props.children}</div>
      </div>
    </div>
  );
}
