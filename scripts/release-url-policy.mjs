export const DEFAULT_RELEASE_REPOSITORY = "space2233/pixnya";

export function releaseRepository(argumentsMap) {
  const repository = (argumentsMap.get("repository") ?? DEFAULT_RELEASE_REPOSITORY).trim();
  const [owner, name, extra] = repository.split("/");
  const validOwner = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/.test(owner ?? "");
  const validName = /^[A-Za-z0-9._-]{1,100}$/.test(name ?? "") && name !== "." && name !== "..";
  if (!validOwner || !validName || extra !== undefined) {
    throw new Error("--repository must be a valid GitHub owner/repository pair");
  }
  return repository;
}

export function validateReleaseBaseUrl(baseUrl, repository) {
  const expectedPrefix = `/${repository}/releases/download/`;
  const tag = baseUrl.pathname.startsWith(expectedPrefix) && baseUrl.pathname.endsWith("/")
    ? baseUrl.pathname.slice(expectedPrefix.length, -1)
    : "";
  if (
    baseUrl.protocol !== "https:" ||
    baseUrl.hostname !== "github.com" ||
    baseUrl.port !== "" ||
    baseUrl.username !== "" ||
    baseUrl.password !== "" ||
    baseUrl.search !== "" ||
    baseUrl.hash !== "" ||
    tag === "" ||
    tag.includes("/")
  ) {
    throw new Error(`--base-url must be an HTTPS github.com Release directory under ${expectedPrefix}`);
  }
}
