export function buildPublicUrl(request: Request, pathname: string) {
  const internalUrl = new URL(request.url);
  const forwardedProto =
    request.headers.get("x-forwarded-proto") ?? internalUrl.protocol.replace(":", "");
  const forwardedHost =
    request.headers.get("x-forwarded-host") ?? request.headers.get("host");

  if (!forwardedHost) {
    return new URL(pathname, internalUrl);
  }

  return new URL(pathname, `${forwardedProto}://${forwardedHost}`);
}
