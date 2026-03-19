import { z } from "zod";

export const SearchQuerySchema = z.object({
  q: z.string().min(1),
  limit: z.coerce.number().int().min(1).max(20).default(10),
  offset: z.coerce.number().int().min(0).default(0),
  lang: z.string().trim().min(1).optional(),
  site: z.string().trim().min(1).optional(),
  freshness: z.enum(["24h", "7d", "30d", "all"]).default("all"),
});

export const SearchResultSchema = z.object({
  id: z.string(),
  title: z.string(),
  url: z.string().url(),
  display_url: z.string(),
  snippet: z.string(),
  language: z.string(),
  last_crawled_at: z.string(),
  score: z.number(),
});

export const SearchResponseSchema = z.object({
  query: z.string(),
  took_ms: z.number().int().nonnegative(),
  total_estimate: z.number().int().nonnegative(),
  next_offset: z.number().int().nonnegative().nullable(),
  results: z.array(SearchResultSchema),
});

export const SuggestResponseSchema = z.object({
  query: z.string(),
  suggestions: z.array(z.string()),
});

export const CreateApiKeyRequestSchema = z.object({
  name: z.string().trim().min(2).max(64),
});

export const ApiKeySchema = z.object({
  id: z.string(),
  name: z.string(),
  preview: z.string(),
  created_at: z.string(),
  revoked_at: z.string().nullable(),
});

export const CreatedApiKeySchema = z.object({
  id: z.string(),
  name: z.string(),
  preview: z.string(),
  token: z.string(),
  created_at: z.string(),
});

export const DeveloperUsageSchema = z.object({
  developer_id: z.string(),
  qps_limit: z.number().int().positive(),
  daily_limit: z.number().int().positive(),
  used_today: z.number().int().nonnegative(),
  keys: z.array(ApiKeySchema),
});

export const CreateCrawlerRequestSchema = z.object({
  name: z.string().trim().min(2).max(64),
});

export const CreatedCrawlerSchema = z.object({
  id: z.string(),
  name: z.string(),
  preview: z.string(),
  key: z.string(),
  created_at: z.string(),
});

export const CrawlerMetadataSchema = z.object({
  id: z.string(),
  name: z.string(),
  preview: z.string(),
  created_at: z.string(),
  revoked_at: z.string().nullable(),
  last_seen_at: z.string().nullable(),
  last_claimed_at: z.string().nullable(),
  jobs_claimed: z.number().int().nonnegative(),
  jobs_reported: z.number().int().nonnegative(),
});

export const CrawlOverviewSchema = z.object({
  developer_id: z.string(),
  frontier_depth: z.number().int().nonnegative(),
  known_urls: z.number().int().nonnegative(),
  in_flight_jobs: z.number().int().nonnegative(),
  indexed_documents: z.number().int().nonnegative(),
  crawlers: z.array(CrawlerMetadataSchema),
});

export const SeedFrontierRequestSchema = z.object({
  urls: z.array(z.string().url()).min(1).max(200),
  source: z.string().trim().min(1).max(128).optional(),
});

export const SeedFrontierResponseSchema = z.object({
  accepted_urls: z.number().int().nonnegative(),
  frontier_depth: z.number().int().nonnegative(),
  known_urls: z.number().int().nonnegative(),
});

export type SearchQuery = z.infer<typeof SearchQuerySchema>;
export type SearchResult = z.infer<typeof SearchResultSchema>;
export type SearchResponse = z.infer<typeof SearchResponseSchema>;
export type SuggestResponse = z.infer<typeof SuggestResponseSchema>;
export type CreateApiKeyRequest = z.infer<typeof CreateApiKeyRequestSchema>;
export type ApiKey = z.infer<typeof ApiKeySchema>;
export type CreatedApiKey = z.infer<typeof CreatedApiKeySchema>;
export type DeveloperUsage = z.infer<typeof DeveloperUsageSchema>;
export type CreateCrawlerRequest = z.infer<typeof CreateCrawlerRequestSchema>;
export type CreatedCrawler = z.infer<typeof CreatedCrawlerSchema>;
export type CrawlerMetadata = z.infer<typeof CrawlerMetadataSchema>;
export type CrawlOverview = z.infer<typeof CrawlOverviewSchema>;
export type SeedFrontierRequest = z.infer<typeof SeedFrontierRequestSchema>;
export type SeedFrontierResponse = z.infer<typeof SeedFrontierResponseSchema>;
