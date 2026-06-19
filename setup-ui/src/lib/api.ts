// Thin client for the Discrakt setup server's JSON endpoints.

export type TraktSubmitResponse = {
  user_code?: string;
  verification_url?: string;
  expires_in?: number;
  interval?: number;
};

export type PlexLoginResponse = {
  authUrl: string;
  code: string;
  expiresIn: number;
  interval: number;
};

export type SetupStatus = {
  status: "idle" | "pending" | "success" | "denied" | "expired" | "error";
  message?: string;
};

async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error((await response.text()) || `Request failed (${response.status})`);
  }
  return (await response.json()) as T;
}

export function submitTrakt(): Promise<TraktSubmitResponse> {
  // The OAuth device flow identifies the account; no username is needed.
  return postJson<TraktSubmitResponse>("/submit", {});
}

export function submitTraktPublic(traktUser: string): Promise<unknown> {
  // Public-profile setup: poll the user's public watching status, no login.
  return postJson("/submit-public", { traktUser });
}

export function submitPlex(input: {
  serverUrl: string;
  token: string;
  username: string;
}): Promise<unknown> {
  return postJson("/submit-plex", input);
}

export function startPlexLogin(): Promise<PlexLoginResponse> {
  return postJson<PlexLoginResponse>("/plex-login/start", {});
}

export async function getStatus(): Promise<SetupStatus> {
  const response = await fetch("/status");
  return (await response.json()) as SetupStatus;
}
