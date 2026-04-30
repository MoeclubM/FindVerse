type RequestOptions = RequestInit & {
  token?: string | null;
};

export async function request<T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const { token, ...fetchOptions } = options;
  const response = await fetch(`/api${path}`, {
    ...fetchOptions,
    headers: {
      Accept: "application/json",
      ...(fetchOptions.body ? { "Content-Type": "application/json" } : {}),
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(fetchOptions.headers ?? {}),
    },
  });

  if (!response.ok) {
    const text = await response.text();
    const error = new Error(text || `Request failed with ${response.status}`);
    (error as Error & { status: number }).status = response.status;
    throw error;
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}
