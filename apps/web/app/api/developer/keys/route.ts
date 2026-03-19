import { NextResponse } from "next/server";

import { createDeveloperKey } from "@/lib/api";
import { getSession } from "@/lib/session";

export async function POST(request: Request) {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  try {
    const body = (await request.json()) as { name?: string };
    const created = await createDeveloperKey(session.id, {
      name: body.name ?? "",
    });
    return NextResponse.json(created, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "Key creation failed" },
      { status: 502 },
    );
  }
}
