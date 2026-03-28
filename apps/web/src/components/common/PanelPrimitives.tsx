import type { ReactNode } from "react";

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
