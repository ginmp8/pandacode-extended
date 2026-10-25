export const MIN_SECRET_CHARS = 32;

/** Fail closed without identifying missing secrets in the public response. */
export const configured = (env: { SESSION_SECRET?: string; GITHUB_CLIENT_ID?: string; GITHUB_CLIENT_SECRET?: string }, forAuth: boolean): boolean =>
  typeof env.SESSION_SECRET === "string" && env.SESSION_SECRET.length >= MIN_SECRET_CHARS &&
  (!forAuth || (!!env.GITHUB_CLIENT_ID && !!env.GITHUB_CLIENT_SECRET));
