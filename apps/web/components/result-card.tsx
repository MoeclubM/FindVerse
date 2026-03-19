import type { SearchResult } from "@findverse/contracts";

type ResultCardProps = {
  result: SearchResult;
};

export function ResultCard({ result }: ResultCardProps) {
  return (
    <article className="result-card">
      <div className="result-meta">
        <span>{result.display_url}</span>
        <span>{result.language.toUpperCase()}</span>
        <span>score {result.score.toFixed(1)}</span>
      </div>
      <h2>
        <a href={result.url} target="_blank" rel="noreferrer">
          {result.title}
        </a>
      </h2>
      <p>{result.snippet}</p>
      <time dateTime={result.last_crawled_at}>
        crawled {new Date(result.last_crawled_at).toLocaleString()}
      </time>
    </article>
  );
}
