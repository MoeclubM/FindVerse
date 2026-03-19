import { NextResponse } from "next/server";

import { loadCrawlOverview } from "@/lib/api";
import { getSession } from "@/lib/session";

export async function GET() {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  try {
    const overview = await loadCrawlOverview(session.id);
    return NextResponse.json(overview);
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Crawler overview load failed",
      },
      { status: 502 },
    );
  }
}
