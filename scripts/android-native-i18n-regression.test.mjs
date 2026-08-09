import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");
const androidResources = "src-tauri/gen/android/app/src/main/res";

function resourceNames(xml) {
  return [...xml.matchAll(/<string\s+name="([^"]+)"/g)]
    .map((match) => match[1])
    .sort();
}

test("Android native sign-in copy has matching English, Simplified Chinese, and Traditional Chinese resources", async () => {
  const [english, simplified, traditional] = await Promise.all([
    read(`${androidResources}/values/strings.xml`),
    read(`${androidResources}/values-zh-rCN/strings.xml`),
    read(`${androidResources}/values-zh-rTW/strings.xml`),
  ]);
  assert.deepEqual(resourceNames(simplified), resourceNames(english));
  assert.deepEqual(resourceNames(traditional), resourceNames(english));
  assert.doesNotMatch(english, /[\u3400-\u9fff]/u);
});

test("Android native sources reference localized resources instead of hard-coded Chinese copy", async () => {
  const [manifest, layout, activity] = await Promise.all([
    read("src-tauri/gen/android/app/src/main/AndroidManifest.xml"),
    read(`${androidResources}/layout/activity_login.xml`),
    read("src-tauri/gen/android/app/src/main/java/io/github/space2233/pixnya/LoginActivity.kt"),
  ]);
  assert.match(manifest, /android:label="@string\/login_activity_title"/);
  assert.match(layout, /android:contentDescription="@string\/login_close_description"/);
  assert.match(layout, /android:text="@string\/login_activity_title"/);
  assert.doesNotMatch(`${manifest}\n${layout}\n${activity}`, /[\u3400-\u9fff]/u);
});
