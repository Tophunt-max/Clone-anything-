import { UploadAnalyzer } from "./Analyzer.tsx";

// Screen 18: upload a native binary and study it with the built-in engine (Clean room reads
// metadata; Deep mode, set on the project, reads code and strings).
export function BinaryAnalyzerPage() {
  return (
    <UploadAnalyzer
      title="Binary analyzer"
      subtitle="Study a native program: its format, functions, strings and imports."
      accept=".exe,.dll,.so,.dylib,.bin,.o,.elf,application/octet-stream"
      note="Executables, shared libraries and object files (ELF, PE, Mach-O, Wasm). Reading code and strings needs Deep mode, which a maintainer turns on in the project's settings."
      steps={[
        { tool: "core_target_triage", label: "Triage (what is it?)", params: (path) => ({ path }) },
        { tool: "core_target_open", label: "Open as the active target", params: (path) => ({ path }) },
        { tool: "native_functions_list", label: "List functions", params: () => ({ limit: 200 }) },
        { tool: "native_strings_browse", label: "Browse strings", params: () => ({ limit: 200 }) },
      ]}
    />
  );
}
