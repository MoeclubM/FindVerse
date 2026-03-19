import { NextResponse } from "next/server";

import { buildPublicUrl } from "@/lib/request-origin";
import { clearSessionCookie } from "@/lib/session";

export async function GET(request: Request) {
  const response = NextResponse.redirect(buildPublicUrl(request, "/developers"));
  clearSessionCookie(response);
  return response;
}
