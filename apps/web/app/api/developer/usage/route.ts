import { NextResponse } from "next/server";

import { loadDeveloperUsage } from "@/lib/api";
import { getSession } from "@/lib/session";

export async function GET() {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  try {
    const usage = await loadDeveloperUsage(session.id);
    return NextResponse.json(usage);
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "Usage load failed" },
      { status: 502 },
    );
  }
}
