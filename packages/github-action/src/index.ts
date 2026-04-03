import { mkdtemp, chmod, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync, type SpawnSyncOptionsWithStringEncoding } from "node:child_process";

type RunOptions = Omit<SpawnSyncOptionsWithStringEncoding, "encoding">;

type PullRequest = {
  number: number;
  base: { ref: string };
  head: {
    ref: string;
    repo: { full_name: string };
  };
  labels?: Array<{ name?: string }>;
};

type PullRequestEvent = {
  action?: string;
  label?: { name?: string };
  pull_request?: PullRequest;
  ref?: string;
};

function getInput(name: string, fallback = ""): string {
  const key = `INPUT_${name.replace(/ /g, "_").toUpperCase()}`;
  const value = process.env[key];
  return value === undefined || value === "" ? fallback : value;
}

function getRequiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function log(message: string): void {
  process.stdout.write(`${message}\n`);
}

function fail(message: string): void {
  process.stderr.write(`${message}\n`);
}

function run(command: string, args: string[], options: RunOptions = {}): string {
  const printable = [command, ...args].join(" ");
  log(`$ ${printable}`);
  const result = spawnSync(command, args, {
    stdio: "pipe",
    encoding: "utf8",
    ...options,
  });

  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  if (result.status !== 0) {
    throw new Error(
      `command failed: ${printable}\nstdout:\n${result.stdout.trimEnd()}\nstderr:\n${result.stderr.trimEnd()}`,
    );
  }
  return result.stdout.trim();
}

async function githubRequest<T>(token: string, requestPath: string): Promise<T> {
  const response = await fetch(`https://api.github.com${requestPath}`, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "User-Agent": "pguilbert-canopy-action",
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });

  if (!response.ok) {
    throw new Error(`GitHub API request failed (${response.status}): ${requestPath}`);
  }

  return (await response.json()) as T;
}

async function listOpenPullRequests(token: string, repository: string): Promise<PullRequest[]> {
  const pulls: PullRequest[] = [];
  for (let page = 1; ; page += 1) {
    const batch = await githubRequest<PullRequest[]>(
      token,
      `/repos/${repository}/pulls?state=open&per_page=100&page=${page}`,
    );
    pulls.push(...batch);
    if (batch.length < 100) {
      return pulls;
    }
  }
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

function impactedLabelsFromEvent(
  eventName: string,
  payload: PullRequestEvent,
  openPrs: PullRequest[],
  labelPrefix: string,
): string[] {
  if (eventName === "push") {
    const pushedRef = payload.ref ?? "";
    const pushedBranch = pushedRef.replace(/^refs\/heads\//, "");
    return unique(
      openPrs
        .filter((pr) => pr.base.ref === pushedBranch)
        .flatMap((pr) => (pr.labels ?? []).map((label) => label.name))
        .filter((name): name is string => typeof name === "string" && name.startsWith(labelPrefix)),
    ).sort();
  }

  if (payload.action === "labeled" || payload.action === "unlabeled") {
    const labelName = payload.label?.name;
    return typeof labelName === "string" && labelName.startsWith(labelPrefix) ? [labelName] : [];
  }

  const prLabels = (payload.pull_request?.labels ?? []).map((label) => label.name);
  return unique(
    prLabels.filter(
      (name): name is string => typeof name === "string" && name.startsWith(labelPrefix),
    ),
  ).sort();
}

function selectPrsForLabel(openPrs: PullRequest[], label: string): PullRequest[] {
  return openPrs
    .filter((pr) => (pr.labels ?? []).some((candidate) => candidate.name === label))
    .sort((left, right) => left.number - right.number);
}

function resolveReleaseTarget(): string {
  const platforms: Record<string, string> = {
    linux: "unknown-linux-gnu",
    darwin: "apple-darwin",
  };
  const architectures: Record<string, string> = {
    x64: "x86_64",
    arm64: "aarch64",
  };

  const osTarget = platforms[process.platform];
  if (!osTarget) {
    throw new Error(`unsupported runner platform: ${process.platform}`);
  }

  const archTarget = architectures[process.arch];
  if (!archTarget) {
    throw new Error(`unsupported runner architecture: ${process.arch}`);
  }

  return `${archTarget}-${osTarget}`;
}

async function downloadFile(url: string, destination: string): Promise<void> {
  const response = await fetch(url, {
    headers: {
      "User-Agent": "pguilbert-canopy-action",
    },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`failed to download ${url}: ${response.status}`);
  }

  const buffer = Buffer.from(await response.arrayBuffer());
  await writeFile(destination, buffer);
}

async function ensureCanopyBinary(version: string): Promise<string> {
  const target = resolveReleaseTarget();
  const actionRepository = process.env.GITHUB_ACTION_REPOSITORY || "pguilbert/canopy";
  const archiveName = `canopy-${target}.tar.gz`;
  const releaseUrl =
    version === "latest"
      ? `https://github.com/${actionRepository}/releases/latest/download/${archiveName}`
      : `https://github.com/${actionRepository}/releases/download/${version}/${archiveName}`;

  const tempDir = await mkdtemp(path.join(os.tmpdir(), "canopy-action-"));
  const archivePath = path.join(tempDir, archiveName);
  const binaryPath = path.join(tempDir, "canopy");
  log(`Downloading canopy from ${releaseUrl}`);
  await downloadFile(releaseUrl, archivePath);
  run("tar", ["-xzf", archivePath, "-C", tempDir]);
  await chmod(binaryPath, 0o755);
  return binaryPath;
}

async function resolveCanopyBinary(): Promise<string> {
  const configuredPath = getInput("canopy-path");
  if (configuredPath) {
    return path.resolve(configuredPath);
  }
  return ensureCanopyBinary(getInput("canopy-version", "latest"));
}

function validatePrGroup(prs: PullRequest[], repository: string, label: string): string {
  const foreignPrs = prs
    .filter((pr) => pr.head.repo.full_name !== repository)
    .map((pr) => pr.number);
  if (foreignPrs.length > 0) {
    throw new Error(
      `label ${label} is applied to fork PRs (${foreignPrs.join(", ")}); only same-repository PRs are supported`,
    );
  }

  const bases = unique(prs.map((pr) => pr.base.ref));
  if (bases.length !== 1) {
    throw new Error(`label ${label} spans multiple base branches: ${bases.join(", ")}`);
  }

  return bases[0] as string;
}

function deleteRemoteBranch(targetBranch: string): void {
  log(`Deleting remote branch ${targetBranch}`);
  try {
    run("git", ["push", "origin", "--delete", targetBranch]);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes("remote ref does not exist") || message.includes("unable to delete")) {
      log(`Remote branch ${targetBranch} does not exist; skipping delete`);
      return;
    }
    throw error;
  }
}

function rebuildBranch(
  canopyBinary: string,
  label: string,
  labelPrefix: string,
  branchPrefix: string,
  repository: string,
  prs: PullRequest[],
): void {
  const suffix = label.slice(labelPrefix.length);
  const targetBranch = `${branchPrefix}${suffix}`;
  const baseRef = validatePrGroup(prs, repository, label);
  const headRefs = unique(prs.map((pr) => pr.head.ref));

  log(`Rebuilding ${targetBranch} from base ${baseRef} for label ${label}`);
  run(canopyBinary, [
    "branch",
    "--remote",
    "origin",
    "--push",
    "--force",
    "--base",
    baseRef,
    targetBranch,
    ...headRefs,
  ]);
}

async function main(): Promise<void> {
  try {
    const token = getInput("github-token", process.env.GITHUB_TOKEN || "");
    if (!token) {
      throw new Error("github-token input is required");
    }

    const repository = getRequiredEnv("GITHUB_REPOSITORY");
    const labelPrefix = getInput("label-prefix", "canopy/");
    const branchPrefix = getInput("branch-prefix", "canopy-");
    const eventName = getRequiredEnv("GITHUB_EVENT_NAME");
    const eventPath = getRequiredEnv("GITHUB_EVENT_PATH");
    const payload = JSON.parse(await readFile(eventPath, "utf8")) as PullRequestEvent;
    const openPrs = await listOpenPullRequests(token, repository);
    const impactedLabels = impactedLabelsFromEvent(eventName, payload, openPrs, labelPrefix);

    if (impactedLabels.length === 0) {
      log("No canopy labels were affected");
      return;
    }

    const canopyBinary = await resolveCanopyBinary();

    for (const label of impactedLabels) {
      const prs = selectPrsForLabel(openPrs, label);
      const suffix = label.slice(labelPrefix.length);
      const targetBranch = `${branchPrefix}${suffix}`;

      if (prs.length === 0) {
        deleteRemoteBranch(targetBranch);
        continue;
      }

      rebuildBranch(canopyBinary, label, labelPrefix, branchPrefix, repository, prs);
    }
  } catch (error) {
    fail(`error: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}

void main();
