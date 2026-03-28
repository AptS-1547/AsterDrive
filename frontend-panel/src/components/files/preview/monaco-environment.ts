import "monaco-editor/esm/vs/editor/common/services/treeViewsDndService";
import "monaco-editor/esm/vs/platform/actionWidget/browser/actionWidget";
import "monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestMemory";
import "monaco-editor/esm/vs/editor/contrib/codelens/browser/codeLensCache";
import "monaco-editor/esm/vs/editor/contrib/inlayHints/browser/inlayHintsController";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

type MonacoWorkerFactory = {
	getWorker: (_moduleId: string, label: string) => Worker;
};

type MonacoLanguageLoader = () => Promise<unknown>;

let configured = false;

const languageLoaders: Record<string, MonacoLanguageLoader> = {
	bat: () =>
		import("monaco-editor/esm/vs/basic-languages/bat/bat.contribution"),
	c: () => import("monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution"),
	clojure: () =>
		import("monaco-editor/esm/vs/basic-languages/clojure/clojure.contribution"),
	coffeescript: () =>
		import("monaco-editor/esm/vs/basic-languages/coffee/coffee.contribution"),
	cpp: () =>
		import("monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution"),
	csharp: () =>
		import("monaco-editor/esm/vs/basic-languages/csharp/csharp.contribution"),
	css: () =>
		import("monaco-editor/esm/vs/basic-languages/css/css.contribution"),
	dart: () =>
		import("monaco-editor/esm/vs/basic-languages/dart/dart.contribution"),
	dockerfile: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/dockerfile/dockerfile.contribution"
		),
	elixir: () =>
		import("monaco-editor/esm/vs/basic-languages/elixir/elixir.contribution"),
	go: () => import("monaco-editor/esm/vs/basic-languages/go/go.contribution"),
	graphql: () =>
		import("monaco-editor/esm/vs/basic-languages/graphql/graphql.contribution"),
	hcl: () =>
		import("monaco-editor/esm/vs/basic-languages/hcl/hcl.contribution"),
	html: () =>
		import("monaco-editor/esm/vs/basic-languages/html/html.contribution"),
	ini: () =>
		import("monaco-editor/esm/vs/basic-languages/ini/ini.contribution"),
	java: () =>
		import("monaco-editor/esm/vs/basic-languages/java/java.contribution"),
	javascript: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution"
		),
	json: () => import("monaco-editor/esm/vs/language/json/monaco.contribution"),
	julia: () =>
		import("monaco-editor/esm/vs/basic-languages/julia/julia.contribution"),
	kotlin: () =>
		import("monaco-editor/esm/vs/basic-languages/kotlin/kotlin.contribution"),
	less: () =>
		import("monaco-editor/esm/vs/basic-languages/less/less.contribution"),
	lua: () =>
		import("monaco-editor/esm/vs/basic-languages/lua/lua.contribution"),
	markdown: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution"
		),
	perl: () =>
		import("monaco-editor/esm/vs/basic-languages/perl/perl.contribution"),
	php: () =>
		import("monaco-editor/esm/vs/basic-languages/php/php.contribution"),
	powershell: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/powershell/powershell.contribution"
		),
	protobuf: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/protobuf/protobuf.contribution"
		),
	python: () =>
		import("monaco-editor/esm/vs/basic-languages/python/python.contribution"),
	r: () => import("monaco-editor/esm/vs/basic-languages/r/r.contribution"),
	restructuredtext: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/restructuredtext/restructuredtext.contribution"
		),
	ruby: () =>
		import("monaco-editor/esm/vs/basic-languages/ruby/ruby.contribution"),
	rust: () =>
		import("monaco-editor/esm/vs/basic-languages/rust/rust.contribution"),
	scala: () =>
		import("monaco-editor/esm/vs/basic-languages/scala/scala.contribution"),
	scss: () =>
		import("monaco-editor/esm/vs/basic-languages/scss/scss.contribution"),
	shell: () =>
		import("monaco-editor/esm/vs/basic-languages/shell/shell.contribution"),
	sol: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/solidity/solidity.contribution"
		),
	sql: () =>
		import("monaco-editor/esm/vs/basic-languages/sql/sql.contribution"),
	swift: () =>
		import("monaco-editor/esm/vs/basic-languages/swift/swift.contribution"),
	systemverilog: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/systemverilog/systemverilog.contribution"
		),
	typescript: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution"
		),
	verilog: () =>
		import(
			"monaco-editor/esm/vs/basic-languages/systemverilog/systemverilog.contribution"
		),
	xml: () =>
		import("monaco-editor/esm/vs/basic-languages/xml/xml.contribution"),
	yaml: () =>
		import("monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution"),
};

const languageAliases: Record<string, string> = {
	tex: "plaintext",
	toml: "plaintext",
};

const languageLoadPromises = new Map<string, Promise<void>>();

export function normalizeMonacoLanguageId(language: string) {
	return languageAliases[language] ?? language;
}

export async function ensureMonacoLanguage(language: string) {
	const normalizedLanguage = normalizeMonacoLanguageId(language);
	if (normalizedLanguage === "plaintext") {
		return normalizedLanguage;
	}

	const existingLoad = languageLoadPromises.get(normalizedLanguage);
	if (existingLoad) {
		await existingLoad;
		return normalizedLanguage;
	}

	const loadLanguage = languageLoaders[normalizedLanguage];
	if (!loadLanguage) {
		return "plaintext";
	}

	const loadPromise = loadLanguage()
		.then(() => {})
		.catch((error) => {
			languageLoadPromises.delete(normalizedLanguage);
			throw error;
		});

	languageLoadPromises.set(normalizedLanguage, loadPromise);
	await loadPromise;
	return normalizedLanguage;
}

export function configureMonacoEnvironment() {
	if (configured) {
		return monaco;
	}

	const globalScope = self as typeof globalThis & {
		MonacoEnvironment?: MonacoWorkerFactory;
	};

	globalScope.MonacoEnvironment = {
		getWorker() {
			return new editorWorker();
		},
	};

	configured = true;
	return monaco;
}

export { monaco };
