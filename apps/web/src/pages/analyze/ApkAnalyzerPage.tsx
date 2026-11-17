import { UploadAnalyzer } from "./Analyzer.tsx";

// Screen 16: upload an Android APK and study it (package, components, classes, resources).
export function ApkAnalyzerPage() {
  return (
    <UploadAnalyzer
      title="APK analyzer"
      subtitle="Study an Android app package with evidence for every finding."
      accept=".apk,.aab,.xapk,application/vnd.android.package-archive"
      note="An Android package (.apk). Only analyze apps you are allowed to. Reading code needs Deep mode, set by a maintainer in the project."
      steps={[
        { tool: "android_package_show", label: "Package (manifest, SDKs, signing)", params: (path) => ({ path }) },
        { tool: "android_components_list", label: "Components (activities, services, receivers)", params: (path) => ({ path, limit: 200 }) },
        { tool: "android_classes_find", label: "Find classes", params: (path) => ({ path, limit: 200 }) },
        { tool: "android_resources_read", label: "Resources", params: (path) => ({ path }) },
      ]}
    />
  );
}
