import { NextResponse } from "next/server";

import { createCrawlerKey } from "@/lib/api";
import { getSession } from "@/lib/session";

export async function POST(request: Request) {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  try {
    const body = (await request.json()) as { name?: string };
    const created = await createCrawlerKey(session.id, {
      name: body.name ?? "",
    });
    return NextResponse.json(created, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Crawler creation failed",
      },
      { status: 502 },
    );
  }
}
