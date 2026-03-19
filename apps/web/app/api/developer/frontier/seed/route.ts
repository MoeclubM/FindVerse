import { NextResponse } from "next/server";

import { seedCrawlerFrontier } from "@/lib/api";
import { getSession } from "@/lib/session";

export async function POST(request: Request) {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  try {
    const body = (await request.json()) as { urls?: string[]; source?: string };
    const seeded = await seedCrawlerFrontier(session.id, {
      urls: body.urls ?? [],
      source: body.source,
    });
    return NextResponse.json(seeded, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Frontier seed failed",
      },
      { status: 502 },
    );
  }
}
