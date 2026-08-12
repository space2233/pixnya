import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const REQUIRED_JOBS = [
  "preflight",
  "rust-advisories",
  "windows",
  "windows-arm64",
  "linux",
  "android (aarch64, aarch64-linux-android, arm64-v8a)",
  "android (armv7, armv7-linux-androideabi, armeabi-v7a)",
];
const REQUIRED_ARTIFACTS = [
  "android-arm64-v8a",
  "android-armeabi-v7a",
  "linux-x64",
  "supply-chain",
  "windows-arm64",
  "windows-x64",
];

const requireValue = (condition, message) => {
  if (!condition) throw new Error(message);
};

export function validateReleaseArtifactRun({ run, jobs, artifacts, runId, repository }) {
  requireValue(/^[1-9][0-9]*$/.test(String(runId)), "prior run id must be a positive integer");
  requireValue(run?.id === Number(runId), "prior run id does not match the GitHub response");
  requireValue(run?.status === "completed", "prior run is not complete");
  requireValue(run?.event === "workflow_dispatch", "prior run was not manually dispatched");
  requireValue(run?.head_branch === "main", "prior run did not build main");
  requireValue(run?.path === ".github/workflows/release.yml", "prior run used a different workflow");
  requireValue(run?.run_attempt === 1, "prior run must be the first immutable attempt");
  requireValue(run?.repository?.full_name === repository, "prior run repository does not match");
  requireValue(run?.head_repository?.full_name === repository, "prior run head repository does not match");
  requireValue(/^[0-9a-f]{40}$/i.test(run?.head_sha ?? ""), "prior run source commit is invalid");

  requireValue(Array.isArray(jobs?.jobs), "prior run jobs are missing");
  for (const name of REQUIRED_JOBS) {
    const matches = jobs.jobs.filter((job) => job?.name === name);
    requireValue(matches.length === 1, `expected exactly one successful prior job: ${name}`);
    requireValue(
      matches[0].status === "completed" && matches[0].conclusion === "success",
      `prior job did not succeed: ${name}`,
    );
  }

  requireValue(Array.isArray(artifacts?.artifacts), "prior run artifacts are missing");
  const artifactNames = artifacts.artifacts.map((artifact) => artifact?.name).sort();
  requireValue(
    JSON.stringify(artifactNames) === JSON.stringify(REQUIRED_ARTIFACTS),
    "prior run does not contain the exact signed artifact set",
  );
  const artifactIds = [];
  for (const name of REQUIRED_ARTIFACTS) {
    const artifact = artifacts.artifacts.find((candidate) => candidate?.name === name);
    requireValue(Number.isInteger(artifact?.id) && artifact.id > 0, `artifact has no stable id: ${name}`);
    requireValue(Number.isInteger(artifact?.size_in_bytes) && artifact.size_in_bytes > 0, `artifact is empty: ${name}`);
    requireValue(artifact?.expired === false, `artifact is expired: ${name}`);
    requireValue(/^sha256:[0-9a-f]{64}$/i.test(artifact?.digest ?? ""), `artifact digest is invalid: ${name}`);
    requireValue(artifact?.workflow_run?.id === Number(runId), `artifact belongs to another run: ${name}`);
    requireValue(artifact?.workflow_run?.head_sha === run.head_sha, `artifact belongs to another commit: ${name}`);
    requireValue(artifact?.workflow_run?.head_branch === "main", `artifact was not built from main: ${name}`);
    requireValue(
      artifact?.workflow_run?.repository_id === run.repository.id &&
        artifact?.workflow_run?.head_repository_id === run.head_repository.id,
      `artifact repository does not match: ${name}`,
    );
    artifactIds.push(artifact.id);
  }

  return {
    artifactIds,
    sourceSha: run.head_sha.toLowerCase(),
  };
}

const parseArguments = (argv) => {
  const values = new Map();
  for (let index = 2; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    requireValue(key?.startsWith("--") && value !== undefined, `invalid argument: ${key ?? ""}`);
    values.set(key, value);
  }
  return values;
};

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = parseArguments(process.argv);
    for (const name of ["--run", "--jobs", "--artifacts", "--run-id", "--repository"]) {
      requireValue(args.has(name), `missing argument: ${name}`);
    }
    const result = validateReleaseArtifactRun({
      run: JSON.parse(readFileSync(args.get("--run"), "utf8")),
      jobs: JSON.parse(readFileSync(args.get("--jobs"), "utf8")),
      artifacts: JSON.parse(readFileSync(args.get("--artifacts"), "utf8")),
      runId: args.get("--run-id"),
      repository: args.get("--repository"),
    });
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
