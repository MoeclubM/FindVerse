import { NextResponse } from "next/server";

import { buildPublicUrl } from "@/lib/request-origin";
import {
  attachSessionCookie,
  clearSessionCookie,
  validateLocalCredentials,
} from "@/lib/session";

export async function POST(request: Request) {
  const formData = await request.formData();
  const username = String(formData.get("username") ?? "");
  const password = String(formData.get("password") ?? "");

  const session = validateLocalCredentials(username, password);
  if (!session) {
    return NextResponse.redirect(
      buildPublicUrl(request, "/developers?auth=failed"),
      303,
    );
  }

  const response = NextResponse.redirect(buildPublicUrl(request, "/developers"), 303);
  clearSessionCookie(response);
  attachSessionCookie(response, session);
  return response;
}
