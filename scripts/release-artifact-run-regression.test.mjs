import assert from "node:assert/strict";
import test from "node:test";

import { validateReleaseArtifactRun } from "./validate-release-artifact-run.mjs";

const runId = "31409694139";
const repository = "space2233/pixnya";
const sourceSha = "ad8e46bd54df7d727ad3be89c31683b7837a4081";
const repositoryId = 280347428;
const requiredJobs = [
  "preflight",
  "rust-advisories",
  "windows",
  "windows-arm64",
  "linux",
  "android (aarch64, aarch64-linux-android, arm64-v8a)",
  "android (armv7, armv7-linux-androideabi, armeabi-v7a)",
];
const requiredArtifacts = [
  "android-arm64-v8a",
  "android-armeabi-v7a",
  "linux-x64",
  "supply-chain",
  "windows-arm64",
  "windows-x64",
];

function fixture() {
  const run = {
    id: Number(runId),
    status: "completed",
    conclusion: "failure",
    event: "workflow_dispatch",
    head_branch: "main",
    head_sha: sourceSha,
    path: ".github/workflows/release.yml",
    run_attempt: 1,
    repository: { id: repositoryId, full_name: repository },
    head_repository: { id: repositoryId, full_name: repository },
  };
  const jobs = {
    jobs: requiredJobs.map((name, index) => ({
      id: index + 1,
      name,
      status: "completed",
      conclusion: "success",
    })),
  };
  const artifacts = {
    artifacts: requiredArtifacts.map((name, index) => ({
      id: index + 10,
      name,
      size_in_bytes: index + 1,
      expired: false,
      digest: `sha256:${String(index).padStart(64, "0")}`,
      workflow_run: {
        id: Number(runId),
        head_branch: "main",
        head_sha: sourceSha,
        repository_id: repositoryId,
        head_repository_id: repositoryId,
      },
    })),
  };
  return { run, jobs, artifacts };
}

test("a failed draft can reuse the exact successful signed artifacts without rebuilding", () => {
  const candidate = fixture();
  assert.deepEqual(
    validateReleaseArtifactRun({ ...candidate, runId, repository }),
    { artifactIds: [10, 11, 12, 13, 14, 15], sourceSha },
  );
});

test("artifact-run recovery rejects a failed platform job or a substituted artifact", () => {
  const failedJob = fixture();
  failedJob.jobs.jobs.find((job) => job.name === "windows-arm64").conclusion = "failure";
  assert.throws(
    () => validateReleaseArtifactRun({ ...failedJob, runId, repository }),
    /prior job did not succeed: windows-arm64/,
  );

  const substitutedArtifact = fixture();
  substitutedArtifact.artifacts.artifacts[0].workflow_run.head_sha = "f".repeat(40);
  assert.throws(
    () => validateReleaseArtifactRun({ ...substitutedArtifact, runId, repository }),
    /artifact belongs to another commit/,
  );
});

test("artifact-run recovery rejects a changed artifact set or trust anchor", () => {
  const extraArtifact = fixture();
  extraArtifact.artifacts.artifacts.push({ name: "unexpected" });
  assert.throws(
    () => validateReleaseArtifactRun({ ...extraArtifact, runId, repository }),
    /exact signed artifact set/,
  );

  const badDigest = fixture();
  badDigest.artifacts.artifacts[0].digest = "sha256:pending";
  assert.throws(
    () => validateReleaseArtifactRun({ ...badDigest, runId, repository }),
    /artifact digest is invalid/,
  );
});
