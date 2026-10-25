// Extended source, issues and security policy belong to the Extended repository.
// Download buttons intentionally continue to target ORIGINAL PandaCode binaries:
// these upstream assets must never be described as Extended installers.
const repoUrl = 'https://github.com/ginmp8/pandacode-extended';
export const originalPandaCode = { repo: 'https://github.com/aicodeking/pandacode' } as const;

export const project = {
  repo: repoUrl,
  clone: `${repoUrl}.git`,
  releases: `${originalPandaCode.repo}/releases/latest`,
  issues: `${repoUrl}/issues`,
  security: `${repoUrl}/security/policy`,
};

// Direct downloads: release assets keep these names, so /releases/latest/download/ always
// points at the newest build. Each has a matching .sha256 beside it.
const latest = `${originalPandaCode.repo}/releases/latest/download`;
export const platformDownloads = [
  { label: 'macOS', detail: 'Apple silicon', href: `${latest}/PandaCode-macOS-arm64.dmg` },
  { label: 'Windows', detail: 'x64', href: `${latest}/PandaCode-Windows-x64-setup.exe` },
  { label: 'Linux', detail: 'AppImage', href: `${latest}/PandaCode-Linux-x86_64.AppImage` },
] as const;

export const distribution = {
  platforms: 'macOS · Windows · Linux',
  isUnsignedPreview: true,
  requirements: 'Apple silicon · macOS 12+',
  previewNotice: 'Not notarized by Apple. macOS may block the first launch.',
  appleSupportUrl: 'https://support.apple.com/en-us/102445',
};
