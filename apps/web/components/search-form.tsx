"use client";

import { useMemo, useState } from "react";

type SearchFormProps = {
  defaultQuery?: string;
  compact?: boolean;
  suggestions?: string[];
};

export function SearchForm({
  defaultQuery = "",
  compact = false,
  suggestions = [],
}: SearchFormProps) {
  const [query, setQuery] = useState(defaultQuery);
  const action = useMemo(() => "/search", []);

  return (
    <div className={compact ? "search-shell search-shell-compact" : "search-shell"}>
      <form action={action} className="search-form">
        <label className="sr-only" htmlFor="query">
          Search the index
        </label>
        <input
          id="query"
          name="q"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="search-input"
          placeholder="Search documents, crawl policies, ranking notes..."
          autoComplete="off"
        />
        <button className="search-button" type="submit">
          Search
        </button>
      </form>

      {suggestions.length > 0 ? (
        <div className="pill-row">
          {suggestions.map((suggestion) => (
            <a
              key={suggestion}
              className="pill"
              href={`/search?q=${encodeURIComponent(suggestion)}`}
            >
              {suggestion}
            </a>
          ))}
        </div>
      ) : null}
    </div>
  );
}
